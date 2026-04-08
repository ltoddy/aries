use std::path::PathBuf;

use anyhow::Result;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct LspArgs {
    operation: String,
    #[serde(rename = "filePath")]
    file_path: Option<PathBuf>,
    line: Option<u32>,
    character: Option<u32>,
    query: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LspOutput {
    pub result: Value,
}

#[derive(thiserror::Error, Debug)]
#[allow(dead_code)]
pub enum LspError {
    #[error("LSP Error: {0}")]
    OperationFailed(String),
}

pub struct LspTool;

impl Tool for LspTool {
    const NAME: &'static str = "lsp";
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
                    "filePath": { "type": "string" },
                    "line": { "type": "number" },
                    "character": { "type": "number" },
                    "query": { "type": "string" }
                },
                "required": ["operation"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        // MVP: Integrating a real LSP client (like rust-analyzer or tsserver) requires
        // setting up an async JSON-RPC client, managing child processes, and syncing
        // document state. This is highly complex for an MVP.
        Ok(LspOutput {
            result: serde_json::json!({
                "error": format!("LSP operation '{}' is not fully implemented in this MVP. To support real LSP, you would need to spawn a language server (e.g., rust-analyzer) and communicate via JSON-RPC.", args.operation)
            }),
        })
    }
}
