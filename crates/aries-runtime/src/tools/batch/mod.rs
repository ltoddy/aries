mod args;
mod error;
mod output;

use aries_communicate::Notifier;
use aries_extension::AgentExtensions;
use aries_init::GlobalContext;
use futures::future::join_all;
use rig::tool::Tool;
use serde_json::Value;
use toasty::Db;

pub use self::args::{BatchArgs, BatchCall, NAME};
pub use self::error::BatchError;
pub use self::output::{BatchOutput, ToolOutput};
use crate::state::State;
use crate::tools::context::ToolContext;
use crate::tools::{agent, codesearch, glob, grep, read, task_output, webfetch, websearch};

const MAX_BATCH_CALLS: usize = 25;

pub struct BatchTool {
    state: State,
    gctx: GlobalContext,
    ctx: ToolContext,
    notifier: Notifier,
    extensions: AgentExtensions,
    db: Db,
}

impl BatchTool {
    pub fn new(
        state: State,
        ctx: ToolContext,
        gctx: GlobalContext,
        notifier: Notifier,
        extensions: AgentExtensions,
        db: Db,
    ) -> Self {
        Self { state, gctx, ctx, notifier, extensions, db }
    }

    async fn dispatch(
        &self,
        tool_name: impl AsRef<str>,
        params: Value,
        context: &mut rig::tool::ToolContext,
    ) -> Result<Value, BatchError> {
        let tool_name = tool_name.as_ref();
        let ctx = self.ctx.clone();

        match tool_name {
            agent::NAME => {
                let args = serde_json::from_value::<agent::AgentArgs>(params)
                    .map_err(|e| BatchError::invalid_parameters(tool_name, e))?;
                let tool = agent::AgentTool::new(
                    self.state.clone(),
                    ctx,
                    self.gctx.clone(),
                    self.notifier.clone(),
                    self.extensions.clone(),
                    self.db.clone(),
                );
                let res = Tool::call(&tool, context, args)
                    .await
                    .map_err(|e| BatchError::tool_execution(tool_name, e))?;
                serde_json::to_value(res).map_err(|e| BatchError::serialize_output(tool_name, e))
            },
            read::NAME => {
                let args = serde_json::from_value::<read::ReadArgs>(params)
                    .map_err(|e| BatchError::invalid_parameters(tool_name, e))?;
                let res = Tool::call(&read::ReadTool::new(ctx), context, args)
                    .await
                    .map_err(|e| BatchError::tool_execution(tool_name, e))?;
                serde_json::to_value(res).map_err(|e| BatchError::serialize_output(tool_name, e))
            },
            glob::NAME => {
                let args = serde_json::from_value::<glob::GlobArgs>(params)
                    .map_err(|e| BatchError::invalid_parameters(tool_name, e))?;
                let res = Tool::call(&glob::GlobTool::new(ctx), context, args)
                    .await
                    .map_err(|e| BatchError::tool_execution(tool_name, e))?;
                serde_json::to_value(res).map_err(|e| BatchError::serialize_output(tool_name, e))
            },
            grep::NAME => {
                let args = serde_json::from_value::<grep::GrepArgs>(params)
                    .map_err(|e| BatchError::invalid_parameters(tool_name, e))?;
                let res = Tool::call(&grep::GrepTool::new(ctx), context, args)
                    .await
                    .map_err(|e| BatchError::tool_execution(tool_name, e))?;
                serde_json::to_value(res).map_err(|e| BatchError::serialize_output(tool_name, e))
            },
            task_output::NAME => {
                let args = serde_json::from_value::<task_output::TaskOutputArgs>(params)
                    .map_err(|e| BatchError::invalid_parameters(tool_name, e))?;
                let res = Tool::call(&task_output::TaskOutputTool::new(ctx), context, args)
                    .await
                    .map_err(|e| BatchError::tool_execution(tool_name, e))?;
                serde_json::to_value(res).map_err(|e| BatchError::serialize_output(tool_name, e))
            },
            webfetch::NAME => {
                let args = serde_json::from_value::<webfetch::WebFetchArgs>(params)
                    .map_err(|e| BatchError::invalid_parameters(tool_name, e))?;
                let res = Tool::call(&webfetch::WebFetchTool::new(), context, args)
                    .await
                    .map_err(|e| BatchError::tool_execution(tool_name, e))?;
                serde_json::to_value(res).map_err(|e| BatchError::serialize_output(tool_name, e))
            },
            websearch::NAME => {
                let args = serde_json::from_value::<websearch::WebSearchArgs>(params)
                    .map_err(|e| BatchError::invalid_parameters(tool_name, e))?;
                let res = Tool::call(&websearch::WebSearchTool::new(), context, args)
                    .await
                    .map_err(|e| BatchError::tool_execution(tool_name, e))?;
                serde_json::to_value(res).map_err(|e| BatchError::serialize_output(tool_name, e))
            },
            codesearch::NAME => {
                let args = serde_json::from_value::<codesearch::CodeSearchArgs>(params)
                    .map_err(|e| BatchError::invalid_parameters(tool_name, e))?;
                let res = Tool::call(&codesearch::CodeSearchTool::new(), context, args)
                    .await
                    .map_err(|e| BatchError::tool_execution(tool_name, e))?;
                serde_json::to_value(res).map_err(|e| BatchError::serialize_output(tool_name, e))
            },
            _ => Err(BatchError::unsupported_tool(tool_name)),
        }
    }
}

impl Tool for BatchTool {
    const NAME: &'static str = NAME;
    type Args = BatchArgs;
    type Output = BatchOutput;
    type Error = BatchError;

    fn description(&self) -> String {
        include_str!("description.md").to_owned()
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "calls": {
                    "type": "array",
                    "description": "All argument values must be known before execution; results from sibling calls cannot be used as inputs.",
                    "maxItems": MAX_BATCH_CALLS,
                    "items": {
                        "type": "object",
                        "properties": {
                            "tool": {
                                "type": "string",
                                "description": "Case-sensitive supported name: Agent, Read, Glob, Grep, WebFetch, WebSearch, CodeSearch or TaskOutput."
                            },
                            "parameters": {
                                "type": "object",
                                "description": "Argument object conforming to the selected tool schema."
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
        context: &mut rig::tool::ToolContext,
        args: Self::Args,
    ) -> Result<Self::Output, Self::Error> {
        if args.calls.len() > MAX_BATCH_CALLS {
            return Err(BatchError::too_many_calls(MAX_BATCH_CALLS, args.calls.len()));
        }

        let mut futures = Vec::new();

        for call in args.calls {
            let tool_name = call.tool;
            let params = call.parameters;
            let mut context = context.clone();
            let future = async move {
                if tool_name == NAME {
                    Err(BatchError::nested_batch())
                } else {
                    self.dispatch(&tool_name, params, &mut context).await
                }
            };
            futures.push(future);
        }

        let executed_results = join_all(futures).await;

        let mut final_results = Vec::new();
        for res in executed_results.into_iter() {
            match res {
                Ok(value) => final_results.push(ToolOutput::success(value)),
                Err(e) => final_results.push(ToolOutput::failed(e)),
            }
        }

        Ok(BatchOutput::new(final_results))
    }
}
