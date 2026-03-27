use anyhow::Result;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Deserialize)]
#[allow(dead_code)]
pub struct BatchCall {
    tool: String,
    parameters: Value,
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub struct BatchArgs {
    calls: Vec<BatchCall>,
}

#[derive(Serialize)]
pub struct BatchOutput {
    results: Vec<Value>,
}

#[derive(thiserror::Error, Debug)]
#[allow(dead_code)]
pub enum BatchError {
    #[error("Batch execution failed: {0}")]
    ExecutionError(String),
}

pub struct BatchTool;

impl Tool for BatchTool {
    const NAME: &'static str = "batch";
    type Error = BatchError;
    type Args = BatchArgs;
    type Output = BatchOutput;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: include_str!("descriptions/batch.txt").to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "calls": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "tool": {
                                    "type": "string",
                                    "description": "The name of the tool to call"
                                },
                                "parameters": {
                                    "type": "object",
                                    "description": "The parameters to pass to the tool"
                                }
                            },
                            "required": ["tool", "parameters"]
                        }
                    }
                },
                "required": ["calls"]
            }),
        }
    }

    async fn call(&self, _args: Self::Args) -> Result<Self::Output, Self::Error> {
        // For the MVP, we just return a message saying it's not fully implemented yet
        // A real implementation would need to access the Agent's tool registry,
        // which is tricky with rig-core's current architecture without a shared state
        // or Arc<RwLock>.

        Ok(BatchOutput {
            results: vec![
                serde_json::json!({"error": "Batch tool parsing is supported, but concurrent execution is not fully implemented in this MVP yet. Please call tools sequentially or rely on the LLM's native parallel tool calling capability."}),
            ],
        })
    }
}
