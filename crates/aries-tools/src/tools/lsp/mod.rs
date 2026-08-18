mod args;
mod error;
mod output;

use std::path::{Path, PathBuf};

use aries_lspclient::{LspResult, SharedLspClient};
use rig::tool::{Tool, ToolContext};
use serde_json::Value;

pub use self::args::{LspArgs, LspOperation};
pub use self::error::LspError;
pub use self::output::LspOutput;

pub const NAME: &str = "Lsp";

pub struct LspTool {
    client: SharedLspClient,
    cwd: PathBuf,
}

impl LspTool {
    pub fn new(client: SharedLspClient, cwd: impl AsRef<Path>) -> Self {
        let cwd = cwd.as_ref().to_path_buf();

        Self { client, cwd }
    }

    fn extract_position_args(
        args: &LspArgs,
        project_dir: impl AsRef<Path>,
    ) -> Result<(PathBuf, u32, u32), LspError> {
        let LspArgs { file_path, line, character, operation, .. } = args;

        match (file_path, line, character) {
            (Some(file_path), Some(line), Some(character)) => {
                let file_path = if file_path.is_absolute() {
                    file_path.clone()
                } else {
                    project_dir.as_ref().join(file_path)
                };

                Ok((file_path, *line, *character))
            },
            _ => Err(LspError::InvalidInput(format!(
                "file_path, line, character is all required for {operation}"
            ))),
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
                    "description": "The LSP operation to perform",
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
                    "description": "The file path to perform the operation on"
                },
                "line": {
                    "type": "number",
                    "description": "The line number to perform the operation at"
                },
                "character": {
                    "type": "number",
                    "description": "The character position to perform the operation at"
                },
                "query": {
                    "type": "string",
                    "description": "The query string for operations that require it"
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
        if let Some(ref file_path) = args.file_path {
            self.client.did_open(file_path).await?;
        }

        let result = match args.operation {
            LspOperation::GoToDefinition => {
                let (file_path, line, character) = Self::extract_position_args(&args, &self.cwd)?;
                self.client.goto_definition(file_path, line, character).await
            },
            LspOperation::FindReferences => {
                let (file_path, line, character) = Self::extract_position_args(&args, &self.cwd)?;
                self.client.find_references(file_path, line, character).await
            },
            LspOperation::Hover => {
                let (file_path, line, character) = Self::extract_position_args(&args, &self.cwd)?;
                self.client.hover(file_path, line, character).await
            },
            LspOperation::DocumentSymbol => {
                let file_path = args.file_path.ok_or_else(|| {
                    LspError::InvalidInput("file_path is required for documentSymbol".to_owned())
                })?;
                let abs_path = if file_path.is_absolute() {
                    file_path.clone()
                } else {
                    self.cwd.join(file_path)
                };
                self.client.document_symbol(abs_path).await
            },
            LspOperation::WorkspaceSymbol => {
                let query = args.query.as_deref().unwrap_or("");
                self.client.workspace_symbol(query).await
            },
            LspOperation::GoToImplementation => {
                let (file_path, line, character) = Self::extract_position_args(&args, &self.cwd)?;
                self.client.goto_implementation(file_path, line, character).await
            },
            LspOperation::PrepareCallHierarchy => {
                let (file_path, line, character) = Self::extract_position_args(&args, &self.cwd)?;
                self.client.prepare_call_hierarchy(file_path, line, character).await
            },
            LspOperation::IncomingCalls => {
                let (file_path, line, character) = Self::extract_position_args(&args, &self.cwd)?;
                let items = self
                    .client
                    .prepare_call_hierarchy(file_path, line, character)
                    .await
                    .map_err(|e| LspError::OperationFailed(e.to_string()))?;
                let item = match items {
                    LspResult::PrepareCallHierarchy(items) => {
                        if let Some(first_item) = items.first() {
                            serde_json::to_value(first_item)
                                .map_err(|e| LspError::OperationFailed(e.to_string()))?
                        } else {
                            return Ok(LspOutput { result: LspResult::IncomingCalls(vec![]) });
                        }
                    },
                    _ => return Ok(LspOutput { result: LspResult::IncomingCalls(vec![]) }),
                };
                self.client.incoming_calls(item).await
            },
            LspOperation::OutgoingCalls => {
                let (file_path, line, character) = Self::extract_position_args(&args, &self.cwd)?;
                let items = self
                    .client
                    .prepare_call_hierarchy(file_path, line, character)
                    .await
                    .map_err(|e| LspError::OperationFailed(e.to_string()))?;
                let item = match items {
                    LspResult::PrepareCallHierarchy(items) => {
                        if let Some(first_item) = items.first() {
                            serde_json::to_value(first_item)
                                .map_err(|e| LspError::OperationFailed(e.to_string()))?
                        } else {
                            return Ok(LspOutput { result: LspResult::OutgoingCalls(vec![]) });
                        }
                    },
                    _ => return Ok(LspOutput { result: LspResult::OutgoingCalls(vec![]) }),
                };
                self.client.outgoing_calls(item).await
            },
        };

        let lsp_result = result.map_err(|e| LspError::OperationFailed(e.to_string()))?;
        Ok(LspOutput { result: lsp_result })
    }
}
