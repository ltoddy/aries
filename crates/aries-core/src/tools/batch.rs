use anyhow::Result;
use aries_config::AriesConfig;
use futures::future::join_all;
use rig::client::CompletionClient;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::language_server::SharedLspClient;
use crate::tools::apply_patch::ApplyPatchTool;
use crate::tools::bash::ShellCommand;
use crate::tools::codesearch::CodeSearchTool;
use crate::tools::edit::EditTool;
use crate::tools::glob::GlobTool;
use crate::tools::grep::GrepTool;
use crate::tools::ls::LsTool;
use crate::tools::lsp::LspTool;
use crate::tools::multiedit::MultiEditTool;
use crate::tools::question::QuestionTool;
use crate::tools::read::ReadFileTool;
use crate::tools::task::TaskTool;
use crate::tools::webfetch::WebFetchTool;
use crate::tools::websearch::WebSearchTool;
use crate::tools::write::WriteFileTool;
use crate::tools::{
    apply_patch, bash, codesearch, edit, glob, grep, ls, lsp, multiedit, question, read, task,
    webfetch, websearch, write,
};

pub const NAME: &str = "batch";

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

pub struct BatchTool<C>
where
    C: CompletionClient,
{
    client: C,
    config: AriesConfig,
    lsp_client: SharedLspClient,
}

impl<C> BatchTool<C>
where
    C: CompletionClient,
{
    pub fn new(client: C, config: AriesConfig, lsp_client: SharedLspClient) -> Self {
        Self { client, config, lsp_client }
    }
}

impl<C> Tool for BatchTool<C>
where
    C: CompletionClient + Clone + Send + Sync + 'static,
{
    const NAME: &'static str = NAME;
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
                if tool_name == bash::NAME {
                    let parsed_args =
                        serde_json::from_value(call.parameters).map_err(|e| e.to_string())?;
                    Tool::call(&ShellCommand, parsed_args)
                        .await
                        .map(|res| serde_json::to_value(res).unwrap())
                        .map_err(|e| e.to_string())
                } else if tool_name == read::NAME {
                    let parsed_args =
                        serde_json::from_value(call.parameters).map_err(|e| e.to_string())?;
                    Tool::call(&ReadFileTool, parsed_args)
                        .await
                        .map(|res| serde_json::to_value(res).unwrap())
                        .map_err(|e| e.to_string())
                } else if tool_name == write::NAME {
                    let parsed_args =
                        serde_json::from_value(call.parameters).map_err(|e| e.to_string())?;
                    Tool::call(&WriteFileTool, parsed_args)
                        .await
                        .map(|res| serde_json::to_value(res).unwrap())
                        .map_err(|e| e.to_string())
                } else if tool_name == glob::NAME {
                    let parsed_args =
                        serde_json::from_value(call.parameters).map_err(|e| e.to_string())?;
                    Tool::call(&GlobTool, parsed_args)
                        .await
                        .map(|res| serde_json::to_value(res).unwrap())
                        .map_err(|e| e.to_string())
                } else if tool_name == grep::NAME {
                    let parsed_args =
                        serde_json::from_value(call.parameters).map_err(|e| e.to_string())?;
                    Tool::call(&GrepTool, parsed_args)
                        .await
                        .map(|res| serde_json::to_value(res).unwrap())
                        .map_err(|e| e.to_string())
                } else if tool_name == ls::NAME {
                    let parsed_args =
                        serde_json::from_value(call.parameters).map_err(|e| e.to_string())?;
                    Tool::call(&LsTool, parsed_args)
                        .await
                        .map(|res| serde_json::to_value(res).unwrap())
                        .map_err(|e| e.to_string())
                } else if tool_name == apply_patch::NAME {
                    let parsed_args =
                        serde_json::from_value(call.parameters).map_err(|e| e.to_string())?;
                    Tool::call(&ApplyPatchTool, parsed_args)
                        .await
                        .map(|res| serde_json::to_value(res).unwrap())
                        .map_err(|e| e.to_string())
                } else if tool_name == multiedit::NAME {
                    let parsed_args =
                        serde_json::from_value(call.parameters).map_err(|e| e.to_string())?;
                    Tool::call(&MultiEditTool, parsed_args)
                        .await
                        .map(|res| serde_json::to_value(res).unwrap())
                        .map_err(|e| e.to_string())
                } else if tool_name == edit::NAME {
                    let parsed_args =
                        serde_json::from_value(call.parameters).map_err(|e| e.to_string())?;
                    Tool::call(&EditTool, parsed_args)
                        .await
                        .map(|res| serde_json::to_value(res).unwrap())
                        .map_err(|e| e.to_string())
                } else if tool_name == question::NAME {
                    let parsed_args =
                        serde_json::from_value(call.parameters).map_err(|e| e.to_string())?;
                    Tool::call(&QuestionTool, parsed_args)
                        .await
                        .map(|res| serde_json::to_value(res).unwrap())
                        .map_err(|e| e.to_string())
                } else if tool_name == task::NAME {
                    let parsed_args =
                        serde_json::from_value(call.parameters).map_err(|e| e.to_string())?;
                    Tool::call(
                        &TaskTool::<C>::new(self.client.clone(), self.config.clone()),
                        parsed_args,
                    )
                    .await
                    .map(|res| serde_json::to_value(res).unwrap())
                    .map_err(|e| e.to_string())
                } else if tool_name == webfetch::NAME {
                    let parsed_args =
                        serde_json::from_value(call.parameters).map_err(|e| e.to_string())?;
                    Tool::call(&WebFetchTool, parsed_args)
                        .await
                        .map(|res| serde_json::to_value(res).unwrap())
                        .map_err(|e| e.to_string())
                } else if tool_name == websearch::NAME {
                    let parsed_args =
                        serde_json::from_value(call.parameters).map_err(|e| e.to_string())?;
                    Tool::call(&WebSearchTool, parsed_args)
                        .await
                        .map(|res| serde_json::to_value(res).unwrap())
                        .map_err(|e| e.to_string())
                } else if tool_name == lsp::NAME {
                    let parsed_args =
                        serde_json::from_value(call.parameters).map_err(|e| e.to_string())?;
                    Tool::call(&LspTool::new(self.lsp_client.clone()), parsed_args)
                        .await
                        .map(|res| serde_json::to_value(res).unwrap())
                        .map_err(|e| e.to_string())
                } else if tool_name == codesearch::NAME {
                    let parsed_args =
                        serde_json::from_value(call.parameters).map_err(|e| e.to_string())?;
                    Tool::call(&CodeSearchTool, parsed_args)
                        .await
                        .map(|res| serde_json::to_value(res).unwrap())
                        .map_err(|e| e.to_string())
                } else if tool_name == NAME {
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
