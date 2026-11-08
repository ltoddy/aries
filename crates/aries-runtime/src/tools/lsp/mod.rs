mod args;
mod error;
mod output;

use std::path::{Path, PathBuf};

use rig::tool::{Tool, ToolContext};
use serde_json::Value;

pub use self::args::{LspArgs, LspOperation};
pub use self::error::LspError;
pub use self::output::LspOutput;
use crate::lspclient::{LspResult, SharedLspClient};

pub const NAME: &str = "Lsp";

pub struct LspTool {
    client: SharedLspClient,
    ctx: crate::tools::ToolContext,
}

impl LspTool {
    pub fn new(client: SharedLspClient, ctx: crate::tools::ToolContext) -> Self {
        Self { client, ctx }
    }

    fn extract_position_args(
        args: &LspArgs,
        project_dir: impl AsRef<Path>,
    ) -> Result<(PathBuf, u32, u32), LspError> {
        let LspArgs { operation, file_path, line, character, query: _ } = args;

        match (file_path, line, character) {
            (Some(file_path), Some(line), Some(character)) => {
                let file_path = if file_path.is_absolute() {
                    file_path.clone()
                } else {
                    project_dir.as_ref().join(file_path)
                };

                Ok((file_path, *line, *character))
            },
            _ => Err(LspError::invalid_input(
                *operation,
                format!("file_path, line, character is all required for {operation}"),
            )),
        }
    }
}

impl Tool for LspTool {
    const NAME: &'static str = NAME;
    type Args = LspArgs;
    type Output = LspOutput;
    type Error = LspError;

    fn description(&self) -> String {
        include_str!("description.md").to_owned()
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "operation": {
                    "type": "string",
                    "description": "goToDefinition/goToImplementation: definition or implementation locations; findReferences: references including declarations; hover: type, signature or docs; documentSymbol: file symbols; workspaceSymbol: workspace symbol search; prepareCallHierarchy: entry items; incomingCalls/outgoingCalls: callers or callees.",
                    "enum": [
                        "goToDefinition",
                        "findReferences",
                        "hover",
                        "documentSymbol",
                        "workspaceSymbol",
                        "goToImplementation",
                        "prepareCallHierarchy",
                        "incomingCalls",
                        "outgoingCalls"
                    ]
                },
                "file_path": {
                    "type": "string",
                    "description": "Target file path, absolute or relative to the current working directory. Required except for workspaceSymbol."
                },
                "line": {
                    "type": "number",
                    "description": "0-based line index as a non-negative integer. Required except for documentSymbol and workspaceSymbol, where it is unused. Subtract 1 from Read line numbers."
                },
                "character": {
                    "type": "number",
                    "description": "0-based UTF-16 code-unit offset within the line, as a non-negative integer; not a byte offset or visual column. Required wherever line is required and unused otherwise."
                },
                "query": {
                    "type": "string",
                    "description": "Symbol search text for workspaceSymbol only; defaults to an empty string. Matching behavior is determined by the language server."
                }
            },
            "required": [
                "operation"
            ]
        })
    }

    async fn call(
        &self,
        _context: &mut ToolContext,
        args: Self::Args,
    ) -> Result<Self::Output, Self::Error> {
        if let Some(file_path) = &args.file_path {
            let file_path =
                if file_path.is_absolute() { file_path } else { &self.ctx.cwd().join(file_path) };
            self.client
                .did_open(file_path)
                .await
                .map_err(|err| LspError::did_open(file_path.clone(), err))?;
        }

        let result = match args.operation {
            LspOperation::GoToDefinition => {
                let (file_path, line, character) =
                    Self::extract_position_args(&args, self.ctx.cwd())?;
                self.client.goto_definition(file_path, line, character).await
            },
            LspOperation::FindReferences => {
                let (file_path, line, character) =
                    Self::extract_position_args(&args, self.ctx.cwd())?;
                self.client.find_references(file_path, line, character).await
            },
            LspOperation::Hover => {
                let (file_path, line, character) =
                    Self::extract_position_args(&args, self.ctx.cwd())?;
                self.client.hover(file_path, line, character).await
            },
            LspOperation::DocumentSymbol => {
                let file_path = args.file_path.ok_or_else(|| {
                    LspError::invalid_input(
                        LspOperation::DocumentSymbol,
                        "file_path is required for documentSymbol",
                    )
                })?;
                let abs_path = if file_path.is_absolute() {
                    file_path.clone()
                } else {
                    self.ctx.cwd().join(file_path)
                };
                self.client.document_symbol(abs_path).await
            },
            LspOperation::WorkspaceSymbol => {
                let query = args.query.as_deref().unwrap_or("");
                self.client.workspace_symbol(query).await
            },
            LspOperation::GoToImplementation => {
                let (file_path, line, character) =
                    Self::extract_position_args(&args, self.ctx.cwd())?;
                self.client.goto_implementation(file_path, line, character).await
            },
            LspOperation::PrepareCallHierarchy => {
                let (file_path, line, character) =
                    Self::extract_position_args(&args, self.ctx.cwd())?;
                self.client.prepare_call_hierarchy(file_path, line, character).await
            },
            LspOperation::IncomingCalls => {
                let (file_path, line, character) =
                    Self::extract_position_args(&args, self.ctx.cwd())?;
                let items =
                    self.client.prepare_call_hierarchy(file_path, line, character).await.map_err(
                        |e| LspError::operation_failed(LspOperation::IncomingCalls, e.to_string()),
                    )?;
                let item = match items {
                    LspResult::PrepareCallHierarchy(items) => {
                        if let Some(first_item) = items.first() {
                            serde_json::to_value(first_item).map_err(|e| {
                                LspError::serialize_call_hierarchy_item(
                                    LspOperation::IncomingCalls,
                                    e,
                                )
                            })?
                        } else {
                            return Ok(LspOutput::new(LspResult::IncomingCalls(vec![])));
                        }
                    },
                    _ => return Ok(LspOutput::new(LspResult::IncomingCalls(vec![]))),
                };
                self.client.incoming_calls(item).await
            },
            LspOperation::OutgoingCalls => {
                let (file_path, line, character) =
                    Self::extract_position_args(&args, self.ctx.cwd())?;
                let items =
                    self.client.prepare_call_hierarchy(file_path, line, character).await.map_err(
                        |e| LspError::operation_failed(LspOperation::OutgoingCalls, e.to_string()),
                    )?;
                let item = match items {
                    LspResult::PrepareCallHierarchy(items) => {
                        if let Some(first_item) = items.first() {
                            serde_json::to_value(first_item).map_err(|e| {
                                LspError::serialize_call_hierarchy_item(
                                    LspOperation::OutgoingCalls,
                                    e,
                                )
                            })?
                        } else {
                            return Ok(LspOutput::new(LspResult::OutgoingCalls(vec![])));
                        }
                    },
                    _ => return Ok(LspOutput::new(LspResult::OutgoingCalls(vec![]))),
                };
                self.client.outgoing_calls(item).await
            },
        };

        let lsp_result =
            result.map_err(|e| LspError::operation_failed(args.operation, e.to_string()))?;
        Ok(LspOutput::new(lsp_result))
    }
}
