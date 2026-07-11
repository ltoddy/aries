mod args;
mod error;
mod output;

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;

use futures::future::join_all;
use rig_core::completion::ToolDefinition;
use rig_core::tool::Tool;
use serde_json::Value;

pub use self::args::{BatchArgs, BatchCall, NAME};
pub use self::error::BatchError;
pub use self::output::BatchOutput;

type DispatchFuture = Pin<Box<dyn Future<Output = Result<Value, String>> + Send>>;
type DispatchFn = dyn Fn(String, Value, PathBuf) -> DispatchFuture + Send + Sync;

pub struct BatchTool {
    cwd: PathBuf,
    dispatch: Box<DispatchFn>,
}

impl BatchTool {
    pub fn new<F>(cwd: PathBuf, dispatch: F) -> Self
    where
        F: Fn(String, Value, PathBuf) -> DispatchFuture + Send + Sync + 'static,
    {
        Self { cwd, dispatch: Box::new(dispatch) }
    }
}

impl Tool for BatchTool {
    const NAME: &'static str = NAME;
    type Error = BatchError;
    type Args = BatchArgs;
    type Output = BatchOutput;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_owned(),
            description: include_str!("description.md").to_owned(),
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
            let tool_name = call.tool.clone();
            let params = call.parameters;
            let cwd = self.cwd.clone();
            let dispatch = &self.dispatch;

            let future = async move {
                if tool_name == NAME {
                    Err("Nested batch calls are not allowed".to_string())
                } else {
                    (dispatch)(tool_name, params, cwd).await
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
