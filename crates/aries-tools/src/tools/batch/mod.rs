mod args;
mod error;
mod output;

use std::path::{Path, PathBuf};

use futures::future::join_all;
use rig_agent::tool::Tool;
use serde_json::Value;

pub use self::args::{BatchArgs, BatchCall, NAME};
pub use self::error::BatchError;
pub use self::output::BatchOutput;
use crate::bash::{BashArgs, BashTool};
use crate::codesearch::{CodeSearchArgs, CodeSearchTool};
use crate::context::ToolContext;
use crate::edit::{EditArgs, EditTool};
use crate::glob::{GlobArgs, GlobTool};
use crate::grep::{GrepArgs, GrepTool};
use crate::ls::{LsArgs, LsTool};
use crate::multiedit::{MultiEditArgs, MultiEditTool};
use crate::question::{AskUserQuestionArgs, AskUserQuestionTool};
use crate::read::{ReadArgs, ReadTool};
use crate::webfetch::{WebFetchArgs, WebFetchTool};
use crate::websearch::{WebSearchArgs, WebSearchTool};
use crate::write::{WriteArgs, WriteTool};
use crate::{
    agent, bash, codesearch, edit, glob, grep, ls, multiedit, question, read, webfetch, websearch,
    write,
};

pub struct BatchTool {
    cwd: PathBuf,
    ctx: ToolContext,
}

impl BatchTool {
    pub fn new(cwd: impl AsRef<Path>, ctx: ToolContext) -> Self {
        Self { cwd: cwd.as_ref().to_path_buf(), ctx }
    }
}

impl Tool for BatchTool {
    const NAME: &'static str = NAME;
    type Error = BatchError;
    type Args = BatchArgs;
    type Output = BatchOutput;

    fn description(&self) -> String {
        include_str!("description.md").to_owned()
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
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
        })
    }

    async fn call(
        &self,
        context: &mut rig_agent::tool::ToolContext,
        args: Self::Args,
    ) -> Result<Self::Output, Self::Error> {
        let mut futures = Vec::new();

        for call in args.calls.into_iter().take(25) {
            let tool_name = call.tool.clone();
            let params = call.parameters;
            let cwd = self.cwd.clone();
            let ctx = self.ctx.clone();

            let mut context = context.clone();
            let future = async move {
                if tool_name == NAME {
                    Err("Nested batch calls are not allowed".to_string())
                } else {
                    dispatch(tool_name, params, cwd, ctx, &mut context).await
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

async fn dispatch(
    tool_name: String,
    params: Value,
    cwd: PathBuf,
    ctx: ToolContext,
    context: &mut rig_agent::tool::ToolContext,
) -> Result<Value, String> {
    match tool_name.as_str() {
        bash::NAME => {
            let args: BashArgs = serde_json::from_value(params).map_err(|e| e.to_string())?;
            Tool::call(&BashTool::new(cwd), context, args)
                .await
                .map(|res| serde_json::to_value(res).unwrap())
                .map_err(|e| e.to_string())
        },
        read::NAME => {
            let args: ReadArgs = serde_json::from_value(params).map_err(|e| e.to_string())?;
            Tool::call(&ReadTool::new(cwd, ctx), context, args)
                .await
                .map(|res| serde_json::to_value(res).unwrap())
                .map_err(|e| e.to_string())
        },
        write::NAME => {
            let args: WriteArgs = serde_json::from_value(params).map_err(|e| e.to_string())?;
            Tool::call(&WriteTool::new(cwd, ctx), context, args)
                .await
                .map(|res| serde_json::to_value(res).unwrap())
                .map_err(|e| e.to_string())
        },
        glob::NAME => {
            let args: GlobArgs = serde_json::from_value(params).map_err(|e| e.to_string())?;
            Tool::call(&GlobTool::new(cwd), context, args)
                .await
                .map(|res| serde_json::to_value(res).unwrap())
                .map_err(|e| e.to_string())
        },
        grep::NAME => {
            let args: GrepArgs = serde_json::from_value(params).map_err(|e| e.to_string())?;
            Tool::call(&GrepTool::new(cwd), context, args)
                .await
                .map(|res| serde_json::to_value(res).unwrap())
                .map_err(|e| e.to_string())
        },
        ls::NAME => {
            let args: LsArgs = serde_json::from_value(params).map_err(|e| e.to_string())?;
            Tool::call(&LsTool::new(cwd), context, args)
                .await
                .map(|res| serde_json::to_value(res).unwrap())
                .map_err(|e| e.to_string())
        },
        multiedit::NAME => {
            let args: MultiEditArgs = serde_json::from_value(params).map_err(|e| e.to_string())?;
            Tool::call(&MultiEditTool::new(cwd, ctx), context, args)
                .await
                .map(|res| serde_json::to_value(res).unwrap())
                .map_err(|e| e.to_string())
        },
        edit::NAME => {
            let args: EditArgs = serde_json::from_value(params).map_err(|e| e.to_string())?;
            Tool::call(&EditTool::new(cwd, ctx), context, args)
                .await
                .map(|res| serde_json::to_value(res).unwrap())
                .map_err(|e| e.to_string())
        },
        question::NAME => {
            let args: AskUserQuestionArgs =
                serde_json::from_value(params).map_err(|e| e.to_string())?;
            Tool::call(&AskUserQuestionTool, context, args)
                .await
                .map(|res| serde_json::to_value(res).unwrap())
                .map_err(|e| e.to_string())
        },
        agent::NAME => Err("AgentTool is not allowed in batch".to_string()),
        webfetch::NAME => {
            let args: WebFetchArgs = serde_json::from_value(params).map_err(|e| e.to_string())?;
            Tool::call(&WebFetchTool, context, args)
                .await
                .map(|res| serde_json::to_value(res).unwrap())
                .map_err(|e| e.to_string())
        },
        websearch::NAME => {
            let args: WebSearchArgs = serde_json::from_value(params).map_err(|e| e.to_string())?;
            Tool::call(&WebSearchTool, context, args)
                .await
                .map(|res| serde_json::to_value(res).unwrap())
                .map_err(|e| e.to_string())
        },
        codesearch::NAME => {
            let args: CodeSearchArgs = serde_json::from_value(params).map_err(|e| e.to_string())?;
            Tool::call(&CodeSearchTool, context, args)
                .await
                .map(|res| serde_json::to_value(res).unwrap())
                .map_err(|e| e.to_string())
        },
        _ => Err(format!("Tool '{}' not found or not supported in batch", tool_name)),
    }
}
