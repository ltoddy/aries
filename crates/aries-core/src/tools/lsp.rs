use std::fmt::{self, Display, Formatter};
use std::path::{Path, PathBuf};

use anyhow::Result;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::language_server::{LspResult, SharedLspClient};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LspOperation {
    GoToDefinition,
    FindReferences,
    Hover,
    DocumentSymbol,
    WorkspaceSymbol,
    GoToImplementation,
    PrepareCallHierarchy,
    IncomingCalls,
    OutgoingCalls,
}

impl Display for LspOperation {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            LspOperation::GoToDefinition => write!(f, "goToDefinition"),
            LspOperation::FindReferences => write!(f, "findReferences"),
            LspOperation::Hover => write!(f, "hover"),
            LspOperation::DocumentSymbol => write!(f, "documentSymbol"),
            LspOperation::WorkspaceSymbol => write!(f, "workspaceSymbol"),
            LspOperation::GoToImplementation => write!(f, "goToImplementation"),
            LspOperation::PrepareCallHierarchy => write!(f, "prepareCallHierarchy"),
            LspOperation::IncomingCalls => write!(f, "incomingCalls"),
            LspOperation::OutgoingCalls => write!(f, "outgoingCalls"),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct LspArgs {
    pub operation: LspOperation,
    pub file_path: Option<PathBuf>,
    pub line: Option<u32>,
    pub character: Option<u32>,
    pub query: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct LspOutput {
    pub result: LspResult,
}

impl Display for LspOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let content = match &self.result {
            LspResult::Definition(locations)
            | LspResult::References(locations)
            | LspResult::Implementation(locations) => locations
                .iter()
                .map(|loc| {
                    format!(
                        "{}:{}:{}",
                        loc.uri.strip_prefix("file://").unwrap_or(&loc.uri),
                        loc.range.start.line + 1,
                        loc.range.start.character + 1
                    )
                })
                .collect::<Vec<_>>()
                .join("\n"),
            LspResult::Hover(hover) => extract_hover_text(&hover.contents),
            LspResult::DocumentSymbol(symbols) | LspResult::WorkspaceSymbol(symbols) => symbols
                .iter()
                .map(|s| {
                    let loc = format!(
                        "{}:{}",
                        s.location.uri.strip_prefix("file://").unwrap_or(&s.location.uri),
                        s.location.range.start.line + 1
                    );
                    format!("{} [{}] {}", s.name, s.kind, loc)
                })
                .collect::<Vec<_>>()
                .join("\n"),
            LspResult::PrepareCallHierarchy(items) => items
                .iter()
                .map(|item| {
                    format!(
                        "{} [{}] {}",
                        item.name,
                        item.kind,
                        item.uri.strip_prefix("file://").unwrap_or(&item.uri)
                    )
                })
                .collect::<Vec<_>>()
                .join("\n"),
            LspResult::IncomingCalls(calls) => calls
                .iter()
                .map(|c| {
                    format!(
                        "{} [{}] {}",
                        c.from.name,
                        c.from.kind,
                        c.from.uri.strip_prefix("file://").unwrap_or(&c.from.uri)
                    )
                })
                .collect::<Vec<_>>()
                .join("\n"),
            LspResult::OutgoingCalls(calls) => calls
                .iter()
                .map(|c| {
                    format!(
                        "{} [{}] {}",
                        c.to.name,
                        c.to.kind,
                        c.to.uri.strip_prefix("file://").unwrap_or(&c.to.uri)
                    )
                })
                .collect::<Vec<_>>()
                .join("\n"),
        };
        write!(f, "{}", content)
    }
}

fn extract_hover_text(contents: &serde_json::Value) -> String {
    match contents {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Object(obj) => {
            obj.get("value").and_then(|v| v.as_str()).unwrap_or_default().to_string()
        },
        serde_json::Value::Array(arr) => arr
            .iter()
            .filter_map(|item| match item {
                serde_json::Value::String(s) => Some(s.as_str()),
                serde_json::Value::Object(obj) => obj.get("value").and_then(|v| v.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

#[derive(thiserror::Error, Debug)]
pub enum LspError {
    #[error("LSP operation failed: {0}")]
    OperationFailed(String),
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    #[error("{0}")]
    IO(#[from] std::io::Error),
}

pub const NAME: &str = "Lsp";

pub struct LspTool {
    client: SharedLspClient,
    cwd: PathBuf,
}

impl LspTool {
    pub fn new(client: SharedLspClient, cwd: PathBuf) -> Self {
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
    type Error = LspError;
    type Args = LspArgs;
    type Output = LspOutput;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: include_str!("descriptions/lsp.txt").to_string(),
            parameters: serde_json::json!({
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
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
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
                    LspError::InvalidInput("file_path is required for documentSymbol".to_string())
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
                            Value::Null
                        }
                    },
                    _ => Value::Null,
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
                            Value::Null
                        }
                    },
                    _ => Value::Null,
                };
                self.client.outgoing_calls(item).await
            },
        };

        let lsp_result = result.map_err(|e| LspError::OperationFailed(e.to_string()))?;
        Ok(LspOutput { result: lsp_result })
    }
}
