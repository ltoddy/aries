use anyhow::Result;
use aries_config::AriesConfig;
use futures::future::join_all;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::tools::*;

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

#[derive(Serialize, Deserialize)]
pub struct BatchOutput {
    pub results: Vec<Value>,
}

#[derive(thiserror::Error, Debug)]
#[allow(dead_code)]
pub enum BatchError {
    #[error("Batch execution failed: {0}")]
    ExecutionError(String),
}

pub struct BatchTool {
    pub config: AriesConfig,
}

impl BatchTool {
    pub fn new(config: AriesConfig) -> Self {
        Self { config }
    }
}

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

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let mut futures = Vec::new();

        for call in args.calls.into_iter().take(25) {
            let future = async move {
                let tool_name = call.tool.as_str();
                if tool_name == ShellCommand::NAME {
                    let parsed_args = serde_json::from_value(call.parameters).map_err(|e| e.to_string())?;
                    ShellCommand
                        .call(parsed_args)
                        .await
                        .map(|res| serde_json::to_value(res).unwrap())
                        .map_err(|e| e.to_string())
                } else if tool_name == ReadFileTool::NAME {
                    let parsed_args = serde_json::from_value(call.parameters).map_err(|e| e.to_string())?;
                    ReadFileTool
                        .call(parsed_args)
                        .await
                        .map(|res| serde_json::to_value(res).unwrap())
                        .map_err(|e| e.to_string())
                } else if tool_name == WriteFileTool::NAME {
                    let parsed_args = serde_json::from_value(call.parameters).map_err(|e| e.to_string())?;
                    WriteFileTool
                        .call(parsed_args)
                        .await
                        .map(|res| serde_json::to_value(res).unwrap())
                        .map_err(|e| e.to_string())
                } else if tool_name == GlobTool::NAME {
                    let parsed_args = serde_json::from_value(call.parameters).map_err(|e| e.to_string())?;
                    GlobTool
                        .call(parsed_args)
                        .await
                        .map(|res| serde_json::to_value(res).unwrap())
                        .map_err(|e| e.to_string())
                } else if tool_name == GrepTool::NAME {
                    let parsed_args = serde_json::from_value(call.parameters).map_err(|e| e.to_string())?;
                    GrepTool
                        .call(parsed_args)
                        .await
                        .map(|res| serde_json::to_value(res).unwrap())
                        .map_err(|e| e.to_string())
                } else if tool_name == LsTool::NAME {
                    let parsed_args = serde_json::from_value(call.parameters).map_err(|e| e.to_string())?;
                    LsTool
                        .call(parsed_args)
                        .await
                        .map(|res| serde_json::to_value(res).unwrap())
                        .map_err(|e| e.to_string())
                } else if tool_name == ApplyPatchTool::NAME {
                    let parsed_args = serde_json::from_value(call.parameters).map_err(|e| e.to_string())?;
                    ApplyPatchTool
                        .call(parsed_args)
                        .await
                        .map(|res| serde_json::to_value(res).unwrap())
                        .map_err(|e| e.to_string())
                } else if tool_name == MultiEditTool::NAME {
                    let parsed_args = serde_json::from_value(call.parameters).map_err(|e| e.to_string())?;
                    MultiEditTool
                        .call(parsed_args)
                        .await
                        .map(|res| serde_json::to_value(res).unwrap())
                        .map_err(|e| e.to_string())
                } else if tool_name == EditTool::NAME {
                    let parsed_args = serde_json::from_value(call.parameters).map_err(|e| e.to_string())?;
                    EditTool
                        .call(parsed_args)
                        .await
                        .map(|res| serde_json::to_value(res).unwrap())
                        .map_err(|e| e.to_string())
                } else if tool_name == QuestionTool::NAME {
                    let parsed_args = serde_json::from_value(call.parameters).map_err(|e| e.to_string())?;
                    QuestionTool
                        .call(parsed_args)
                        .await
                        .map(|res| serde_json::to_value(res).unwrap())
                        .map_err(|e| e.to_string())
                } else if tool_name == TaskTool::NAME {
                    let parsed_args = serde_json::from_value(call.parameters).map_err(|e| e.to_string())?;
                    TaskTool::new(self.config.clone())
                        .call(parsed_args)
                        .await
                        .map(|res| serde_json::to_value(res).unwrap())
                        .map_err(|e| e.to_string())
                } else if tool_name == WebFetchTool::NAME {
                    let parsed_args = serde_json::from_value(call.parameters).map_err(|e| e.to_string())?;
                    WebFetchTool
                        .call(parsed_args)
                        .await
                        .map(|res| serde_json::to_value(res).unwrap())
                        .map_err(|e| e.to_string())
                } else if tool_name == WebSearchTool::NAME {
                    let parsed_args = serde_json::from_value(call.parameters).map_err(|e| e.to_string())?;
                    WebSearchTool
                        .call(parsed_args)
                        .await
                        .map(|res| serde_json::to_value(res).unwrap())
                        .map_err(|e| e.to_string())
                } else if tool_name == LspTool::NAME {
                    let parsed_args = serde_json::from_value(call.parameters).map_err(|e| e.to_string())?;
                    LspTool
                        .call(parsed_args)
                        .await
                        .map(|res| serde_json::to_value(res).unwrap())
                        .map_err(|e| e.to_string())
                } else if tool_name == CodeSearchTool::NAME {
                    let parsed_args = serde_json::from_value(call.parameters).map_err(|e| e.to_string())?;
                    CodeSearchTool
                        .call(parsed_args)
                        .await
                        .map(|res| serde_json::to_value(res).unwrap())
                        .map_err(|e| e.to_string())
                } else if tool_name == BatchTool::NAME {
                    Err("Nested batch calls are not allowed".to_string())
                } else {
                    Err(format!("Tool '{}' not found or not supported in batch", tool_name))
                }
            };
            futures.push(future);
        }

        let executed_results = join_all(futures).await;

        let mut final_results = Vec::new();
        for res in executed_results.into_iter() {
            match res {
                Ok(value) => {
                    final_results.push(serde_json::json!({
                        "success": true,
                        "result": value
                    }));
                },
                Err(e) => {
                    final_results.push(serde_json::json!({
                        "success": false,
                        "error": e
                    }));
                },
            }
        }

        Ok(BatchOutput { results: final_results })
    }
}
