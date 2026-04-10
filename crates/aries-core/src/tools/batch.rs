use std::marker::PhantomData;

use anyhow::Result;
use aries_config::AriesConfig;
use futures::future::join_all;
use rig::agent::PromptHook;
use rig::completion;
use rig::completion::ToolDefinition;
use rig::providers::{azure, openai};
use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::tools::*;

#[derive(Debug, Deserialize, Serialize)]
pub struct BatchCall {
    pub tool: String,
    pub parameters: Value,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct BatchArgs {
    pub calls: Vec<BatchCall>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct BatchOutput {
    pub results: Vec<Value>,
}

#[derive(thiserror::Error, Debug)]
pub enum BatchError {
    #[error("Batch execution failed: {0}")]
    ExecutionError(String),
}

pub struct BatchTool<M, P = ()>
where
    M: completion::CompletionModel,
    P: PromptHook<M>,
{
    config: AriesConfig,
    task_hook: P,
    _phantom: PhantomData<M>,
}

impl<M, P> BatchTool<M, P>
where
    M: completion::CompletionModel,
    P: PromptHook<M>,
{
    pub fn new(config: AriesConfig, task_hook: P) -> Self {
        Self { config, task_hook, _phantom: Default::default() }
    }
}

impl<P> Tool for BatchTool<openai::CompletionModel, P>
where
    P: PromptHook<openai::CompletionModel> + 'static,
{
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
                    Tool::call(&ShellCommand, parsed_args)
                        .await
                        .map(|res| serde_json::to_value(res).unwrap())
                        .map_err(|e| e.to_string())
                } else if tool_name == ReadFileTool::NAME {
                    let parsed_args = serde_json::from_value(call.parameters).map_err(|e| e.to_string())?;
                    Tool::call(&ReadFileTool, parsed_args)
                        .await
                        .map(|res| serde_json::to_value(res).unwrap())
                        .map_err(|e| e.to_string())
                } else if tool_name == WriteFileTool::NAME {
                    let parsed_args = serde_json::from_value(call.parameters).map_err(|e| e.to_string())?;
                    Tool::call(&WriteFileTool, parsed_args)
                        .await
                        .map(|res| serde_json::to_value(res).unwrap())
                        .map_err(|e| e.to_string())
                } else if tool_name == GlobTool::NAME {
                    let parsed_args = serde_json::from_value(call.parameters).map_err(|e| e.to_string())?;
                    Tool::call(&GlobTool, parsed_args)
                        .await
                        .map(|res| serde_json::to_value(res).unwrap())
                        .map_err(|e| e.to_string())
                } else if tool_name == GrepTool::NAME {
                    let parsed_args = serde_json::from_value(call.parameters).map_err(|e| e.to_string())?;
                    Tool::call(&GrepTool, parsed_args)
                        .await
                        .map(|res| serde_json::to_value(res).unwrap())
                        .map_err(|e| e.to_string())
                } else if tool_name == LsTool::NAME {
                    let parsed_args = serde_json::from_value(call.parameters).map_err(|e| e.to_string())?;
                    Tool::call(&LsTool, parsed_args)
                        .await
                        .map(|res| serde_json::to_value(res).unwrap())
                        .map_err(|e| e.to_string())
                } else if tool_name == ApplyPatchTool::NAME {
                    let parsed_args = serde_json::from_value(call.parameters).map_err(|e| e.to_string())?;
                    Tool::call(&ApplyPatchTool, parsed_args)
                        .await
                        .map(|res| serde_json::to_value(res).unwrap())
                        .map_err(|e| e.to_string())
                } else if tool_name == MultiEditTool::NAME {
                    let parsed_args = serde_json::from_value(call.parameters).map_err(|e| e.to_string())?;
                    Tool::call(&MultiEditTool, parsed_args)
                        .await
                        .map(|res| serde_json::to_value(res).unwrap())
                        .map_err(|e| e.to_string())
                } else if tool_name == EditTool::NAME {
                    let parsed_args = serde_json::from_value(call.parameters).map_err(|e| e.to_string())?;
                    Tool::call(&EditTool, parsed_args)
                        .await
                        .map(|res| serde_json::to_value(res).unwrap())
                        .map_err(|e| e.to_string())
                } else if tool_name == QuestionTool::NAME {
                    let parsed_args = serde_json::from_value(call.parameters).map_err(|e| e.to_string())?;
                    Tool::call(&QuestionTool, parsed_args)
                        .await
                        .map(|res| serde_json::to_value(res).unwrap())
                        .map_err(|e| e.to_string())
                } else if tool_name == TaskTool::<openai::CompletionModel, P>::NAME {
                    let parsed_args = serde_json::from_value(call.parameters).map_err(|e| e.to_string())?;
                    Tool::call(
                        &TaskTool::<openai::CompletionModel, P>::new(self.config.clone(), self.task_hook.clone()),
                        parsed_args,
                    )
                    .await
                    .map(|res| serde_json::to_value(res).unwrap())
                    .map_err(|e| e.to_string())
                } else if tool_name == WebFetchTool::NAME {
                    let parsed_args = serde_json::from_value(call.parameters).map_err(|e| e.to_string())?;
                    Tool::call(&WebFetchTool, parsed_args)
                        .await
                        .map(|res| serde_json::to_value(res).unwrap())
                        .map_err(|e| e.to_string())
                } else if tool_name == WebSearchTool::NAME {
                    let parsed_args = serde_json::from_value(call.parameters).map_err(|e| e.to_string())?;
                    Tool::call(&WebSearchTool, parsed_args)
                        .await
                        .map(|res| serde_json::to_value(res).unwrap())
                        .map_err(|e| e.to_string())
                } else if tool_name == LspTool::NAME {
                    let parsed_args = serde_json::from_value(call.parameters).map_err(|e| e.to_string())?;
                    Tool::call(&LspTool, parsed_args)
                        .await
                        .map(|res| serde_json::to_value(res).unwrap())
                        .map_err(|e| e.to_string())
                } else if tool_name == CodeSearchTool::NAME {
                    let parsed_args = serde_json::from_value(call.parameters).map_err(|e| e.to_string())?;
                    Tool::call(&CodeSearchTool, parsed_args)
                        .await
                        .map(|res| serde_json::to_value(res).unwrap())
                        .map_err(|e| e.to_string())
                } else if tool_name == BatchTool::<openai::CompletionModel, P>::NAME {
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

impl<P> Tool for BatchTool<azure::CompletionModel, P>
where
    P: PromptHook<azure::CompletionModel> + 'static,
{
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
                    Tool::call(&ShellCommand, parsed_args)
                        .await
                        .map(|res| serde_json::to_value(res).unwrap())
                        .map_err(|e| e.to_string())
                } else if tool_name == ReadFileTool::NAME {
                    let parsed_args = serde_json::from_value(call.parameters).map_err(|e| e.to_string())?;
                    Tool::call(&ReadFileTool, parsed_args)
                        .await
                        .map(|res| serde_json::to_value(res).unwrap())
                        .map_err(|e| e.to_string())
                } else if tool_name == WriteFileTool::NAME {
                    let parsed_args = serde_json::from_value(call.parameters).map_err(|e| e.to_string())?;
                    Tool::call(&WriteFileTool, parsed_args)
                        .await
                        .map(|res| serde_json::to_value(res).unwrap())
                        .map_err(|e| e.to_string())
                } else if tool_name == GlobTool::NAME {
                    let parsed_args = serde_json::from_value(call.parameters).map_err(|e| e.to_string())?;
                    Tool::call(&GlobTool, parsed_args)
                        .await
                        .map(|res| serde_json::to_value(res).unwrap())
                        .map_err(|e| e.to_string())
                } else if tool_name == GrepTool::NAME {
                    let parsed_args = serde_json::from_value(call.parameters).map_err(|e| e.to_string())?;
                    Tool::call(&GrepTool, parsed_args)
                        .await
                        .map(|res| serde_json::to_value(res).unwrap())
                        .map_err(|e| e.to_string())
                } else if tool_name == LsTool::NAME {
                    let parsed_args = serde_json::from_value(call.parameters).map_err(|e| e.to_string())?;
                    Tool::call(&LsTool, parsed_args)
                        .await
                        .map(|res| serde_json::to_value(res).unwrap())
                        .map_err(|e| e.to_string())
                } else if tool_name == ApplyPatchTool::NAME {
                    let parsed_args = serde_json::from_value(call.parameters).map_err(|e| e.to_string())?;
                    Tool::call(&ApplyPatchTool, parsed_args)
                        .await
                        .map(|res| serde_json::to_value(res).unwrap())
                        .map_err(|e| e.to_string())
                } else if tool_name == MultiEditTool::NAME {
                    let parsed_args = serde_json::from_value(call.parameters).map_err(|e| e.to_string())?;
                    Tool::call(&MultiEditTool, parsed_args)
                        .await
                        .map(|res| serde_json::to_value(res).unwrap())
                        .map_err(|e| e.to_string())
                } else if tool_name == EditTool::NAME {
                    let parsed_args = serde_json::from_value(call.parameters).map_err(|e| e.to_string())?;
                    Tool::call(&EditTool, parsed_args)
                        .await
                        .map(|res| serde_json::to_value(res).unwrap())
                        .map_err(|e| e.to_string())
                } else if tool_name == QuestionTool::NAME {
                    let parsed_args = serde_json::from_value(call.parameters).map_err(|e| e.to_string())?;
                    Tool::call(&QuestionTool, parsed_args)
                        .await
                        .map(|res| serde_json::to_value(res).unwrap())
                        .map_err(|e| e.to_string())
                } else if tool_name == TaskTool::<azure::CompletionModel, P>::NAME {
                    let parsed_args = serde_json::from_value(call.parameters).map_err(|e| e.to_string())?;
                    Tool::call(
                        &TaskTool::<azure::CompletionModel, P>::new(self.config.clone(), self.task_hook.clone()),
                        parsed_args,
                    )
                    .await
                    .map(|res| serde_json::to_value(res).unwrap())
                    .map_err(|e| e.to_string())
                } else if tool_name == WebFetchTool::NAME {
                    let parsed_args = serde_json::from_value(call.parameters).map_err(|e| e.to_string())?;
                    Tool::call(&WebFetchTool, parsed_args)
                        .await
                        .map(|res| serde_json::to_value(res).unwrap())
                        .map_err(|e| e.to_string())
                } else if tool_name == WebSearchTool::NAME {
                    let parsed_args = serde_json::from_value(call.parameters).map_err(|e| e.to_string())?;
                    Tool::call(&WebSearchTool, parsed_args)
                        .await
                        .map(|res| serde_json::to_value(res).unwrap())
                        .map_err(|e| e.to_string())
                } else if tool_name == LspTool::NAME {
                    let parsed_args = serde_json::from_value(call.parameters).map_err(|e| e.to_string())?;
                    Tool::call(&LspTool, parsed_args)
                        .await
                        .map(|res| serde_json::to_value(res).unwrap())
                        .map_err(|e| e.to_string())
                } else if tool_name == CodeSearchTool::NAME {
                    let parsed_args = serde_json::from_value(call.parameters).map_err(|e| e.to_string())?;
                    Tool::call(&CodeSearchTool, parsed_args)
                        .await
                        .map(|res| serde_json::to_value(res).unwrap())
                        .map_err(|e| e.to_string())
                } else if tool_name == BatchTool::<azure::CompletionModel, P>::NAME {
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
