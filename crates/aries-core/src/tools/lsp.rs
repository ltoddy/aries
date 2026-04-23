use std::env;
use std::path::{Path, PathBuf};

use anyhow::Result;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::language_server::SharedLspClient;

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
    pub result: Value,
}

#[derive(thiserror::Error, Debug)]
pub enum LspError {
    #[error("LSP Error: {0}")]
    OperationFailed(String),
}

pub const NAME: &str = "lsp";

pub struct LspTool {
    client: SharedLspClient,
}

impl LspTool {
    pub fn new(client: SharedLspClient) -> Self {
        Self { client }
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
                        "enum": [
                            "goToDefinition", "findReferences", "hover",
                            "documentSymbol", "workspaceSymbol", "goToImplementation",
                            "prepareCallHierarchy", "incomingCalls", "outgoingCalls"
                        ]
                    },
                    "file_path": { "type": "string" },
                    "line": { "type": "number" },
                    "character": { "type": "number" },
                    "query": { "type": "string" }
                },
                "required": ["operation"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let project_dir = env::current_dir().map_err(|e| {
            LspError::OperationFailed(format!("Failed to get current directory: {}", e))
        })?;

        if let Some(ref file_path) = args.file_path {
            let abs_path = if file_path.is_absolute() {
                file_path.clone()
            } else {
                project_dir.join(file_path)
            };
            self.client.did_open(&abs_path).await.map_err(|e| {
                LspError::OperationFailed(format!("Failed to open document: {}", e))
            })?;
        }

        let result = match args.operation {
            LspOperation::GoToDefinition => {
                let (file_path, line, character) = extract_position_args(&args, &project_dir)?;
                self.client.goto_definition(&file_path, line, character).await
            },
            LspOperation::FindReferences => {
                let (file_path, line, character) = extract_position_args(&args, &project_dir)?;
                self.client.find_references(&file_path, line, character).await
            },
            LspOperation::Hover => {
                let (file_path, line, character) = extract_position_args(&args, &project_dir)?;
                self.client.hover(&file_path, line, character).await
            },
            LspOperation::DocumentSymbol => {
                let file_path = args.file_path.as_ref().ok_or_else(|| {
                    LspError::OperationFailed(
                        "file_path is required for documentSymbol".to_string(),
                    )
                })?;
                let abs_path = if file_path.is_absolute() {
                    file_path.clone()
                } else {
                    project_dir.join(file_path)
                };
                self.client.document_symbol(&abs_path).await
            },
            LspOperation::WorkspaceSymbol => {
                let query = args.query.as_deref().unwrap_or("");
                self.client.workspace_symbol(query).await
            },
            LspOperation::GoToImplementation => {
                let (file_path, line, character) = extract_position_args(&args, &project_dir)?;
                self.client.goto_implementation(&file_path, line, character).await
            },
            LspOperation::PrepareCallHierarchy => {
                let (file_path, line, character) = extract_position_args(&args, &project_dir)?;
                self.client.prepare_call_hierarchy(&file_path, line, character).await
            },
            LspOperation::IncomingCalls => {
                let (file_path, line, character) = extract_position_args(&args, &project_dir)?;
                let items = self
                    .client
                    .prepare_call_hierarchy(&file_path, line, character)
                    .await
                    .map_err(|e| LspError::OperationFailed(e.to_string()))?;
                let item = if let Some(arr) = items.as_array() {
                    arr.first().cloned().unwrap_or(Value::Null)
                } else {
                    items
                };
                self.client.incoming_calls(item).await
            },
            LspOperation::OutgoingCalls => {
                let (file_path, line, character) = extract_position_args(&args, &project_dir)?;
                let items = self
                    .client
                    .prepare_call_hierarchy(&file_path, line, character)
                    .await
                    .map_err(|e| LspError::OperationFailed(e.to_string()))?;
                let item = if let Some(arr) = items.as_array() {
                    arr.first().cloned().unwrap_or(Value::Null)
                } else {
                    items
                };
                self.client.outgoing_calls(item).await
            },
        };

        let value = result.map_err(|e| LspError::OperationFailed(e.to_string()))?;
        Ok(LspOutput { result: value })
    }
}

fn extract_position_args(
    args: &LspArgs,
    project_dir: &Path,
) -> Result<(PathBuf, u32, u32), LspError> {
    let file_path = args.file_path.as_ref().ok_or_else(|| {
        LspError::OperationFailed(format!(
            "file_path is required for {:?} operation",
            args.operation
        ))
    })?;
    let abs_path =
        if file_path.is_absolute() { file_path.clone() } else { project_dir.join(file_path) };
    let line = args.line.ok_or_else(|| {
        LspError::OperationFailed(format!("line is required for {:?} operation", args.operation))
    })?;
    let character = args.character.ok_or_else(|| {
        LspError::OperationFailed(format!(
            "character is required for {:?} operation",
            args.operation
        ))
    })?;
    Ok((abs_path, line, character))
}
