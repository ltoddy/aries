mod args;
mod error;
mod output;

use std::path::{Path, PathBuf};

use aries_event::Notifier;
use aries_extension::AgentExtensions;
use futures::future::join_all;
use rig::client::AgentClientExt;
use rig::tool::Tool;
use serde_json::Value;

pub use self::args::{BatchArgs, BatchCall, NAME};
pub use self::error::BatchError;
pub use self::output::{BatchOutput, ToolOutput};
use crate::context::ToolContext;
use crate::{
    agent, bash, codesearch, edit, glob, grep, monitor, multiedit, question, read, task_output,
    task_stop, webfetch, websearch, write,
};

pub struct BatchTool<C>
where
    C: AgentClientExt,
{
    client: C,
    model: String,
    cwd: PathBuf,
    ctx: ToolContext,
    notifier: Notifier,
    extensions: AgentExtensions,
}

impl<C> BatchTool<C>
where
    C: AgentClientExt + Clone + Sync + Send + 'static,
{
    pub fn new(
        client: C,
        model: impl Into<String>,
        cwd: impl AsRef<Path>,
        ctx: ToolContext,
        notifier: Notifier,
        extensions: AgentExtensions,
    ) -> Self {
        let model = model.into();
        let cwd = cwd.as_ref();

        Self { client, model, cwd: cwd.to_owned(), ctx, notifier, extensions }
    }

    async fn dispatch(
        &self,
        tool_name: String,
        params: Value,
        context: &mut rig::tool::ToolContext,
    ) -> Result<Value, BatchError> {
        let cwd = &self.cwd;
        let ctx = self.ctx.clone();

        match tool_name.as_str() {
            agent::NAME => {
                let args = serde_json::from_value::<agent::AgentArgs>(params)
                    .map_err(|e| BatchError::invalid_parameters(tool_name.clone(), e))?;
                let res = Tool::call(
                    &agent::AgentTool::new(
                        self.client.clone(),
                        &self.model,
                        cwd,
                        Notifier::clone(&self.notifier),
                        self.extensions.clone(),
                    ),
                    context,
                    args,
                )
                .await
                .map_err(|e| BatchError::tool_execution(tool_name.clone(), e))?;
                serde_json::to_value(res)
                    .map_err(|e| BatchError::serialize_output(tool_name.clone(), e))
            },
            bash::NAME => {
                let args = serde_json::from_value::<bash::BashArgs>(params)
                    .map_err(|e| BatchError::invalid_parameters(tool_name.clone(), e))?;
                let res = Tool::call(&bash::BashTool::new(cwd, ctx), context, args)
                    .await
                    .map_err(|e| BatchError::tool_execution(tool_name.clone(), e))?;
                serde_json::to_value(res)
                    .map_err(|e| BatchError::serialize_output(tool_name.clone(), e))
            },
            read::NAME => {
                let args = serde_json::from_value::<read::ReadArgs>(params)
                    .map_err(|e| BatchError::invalid_parameters(tool_name.clone(), e))?;
                let res = Tool::call(&read::ReadTool::new(cwd, ctx), context, args)
                    .await
                    .map_err(|e| BatchError::tool_execution(tool_name.clone(), e))?;
                serde_json::to_value(res)
                    .map_err(|e| BatchError::serialize_output(tool_name.clone(), e))
            },
            write::NAME => {
                let args = serde_json::from_value::<write::WriteArgs>(params)
                    .map_err(|e| BatchError::invalid_parameters(tool_name.clone(), e))?;
                let res = Tool::call(&write::WriteTool::new(cwd, ctx), context, args)
                    .await
                    .map_err(|e| BatchError::tool_execution(tool_name.clone(), e))?;
                serde_json::to_value(res)
                    .map_err(|e| BatchError::serialize_output(tool_name.clone(), e))
            },
            glob::NAME => {
                let args = serde_json::from_value::<glob::GlobArgs>(params)
                    .map_err(|e| BatchError::invalid_parameters(tool_name.clone(), e))?;
                let res = Tool::call(&glob::GlobTool::new(cwd), context, args)
                    .await
                    .map_err(|e| BatchError::tool_execution(tool_name.clone(), e))?;
                serde_json::to_value(res)
                    .map_err(|e| BatchError::serialize_output(tool_name.clone(), e))
            },
            grep::NAME => {
                let args = serde_json::from_value::<grep::GrepArgs>(params)
                    .map_err(|e| BatchError::invalid_parameters(tool_name.clone(), e))?;
                let res = Tool::call(&grep::GrepTool::new(cwd), context, args)
                    .await
                    .map_err(|e| BatchError::tool_execution(tool_name.clone(), e))?;
                serde_json::to_value(res)
                    .map_err(|e| BatchError::serialize_output(tool_name.clone(), e))
            },
            monitor::NAME => {
                let args = serde_json::from_value::<monitor::MonitorArgs>(params)
                    .map_err(|e| BatchError::invalid_parameters(tool_name.clone(), e))?;
                let res = Tool::call(&monitor::MonitorTool::new(cwd, ctx), context, args)
                    .await
                    .map_err(|e| BatchError::tool_execution(tool_name.clone(), e))?;
                serde_json::to_value(res)
                    .map_err(|e| BatchError::serialize_output(tool_name.clone(), e))
            },
            multiedit::NAME => {
                let args = serde_json::from_value::<multiedit::MultiEditArgs>(params)
                    .map_err(|e| BatchError::invalid_parameters(tool_name.clone(), e))?;
                let res = Tool::call(&multiedit::MultiEditTool::new(cwd, ctx), context, args)
                    .await
                    .map_err(|e| BatchError::tool_execution(tool_name.clone(), e))?;
                serde_json::to_value(res)
                    .map_err(|e| BatchError::serialize_output(tool_name.clone(), e))
            },
            edit::NAME => {
                let args = serde_json::from_value::<edit::EditArgs>(params)
                    .map_err(|e| BatchError::invalid_parameters(tool_name.clone(), e))?;
                let res = Tool::call(&edit::EditTool::new(cwd, ctx), context, args)
                    .await
                    .map_err(|e| BatchError::tool_execution(tool_name.clone(), e))?;
                serde_json::to_value(res)
                    .map_err(|e| BatchError::serialize_output(tool_name.clone(), e))
            },
            question::NAME => {
                let args = serde_json::from_value::<question::AskUserQuestionArgs>(params)
                    .map_err(|e| BatchError::invalid_parameters(tool_name.clone(), e))?;
                let res = Tool::call(&question::AskUserQuestionTool, context, args)
                    .await
                    .map_err(|e| BatchError::tool_execution(tool_name.clone(), e))?;
                serde_json::to_value(res)
                    .map_err(|e| BatchError::serialize_output(tool_name.clone(), e))
            },
            task_output::NAME => {
                let args = serde_json::from_value::<task_output::TaskOutputArgs>(params)
                    .map_err(|e| BatchError::invalid_parameters(tool_name.clone(), e))?;
                let res = Tool::call(&task_output::TaskOutputTool::new(ctx), context, args)
                    .await
                    .map_err(|e| BatchError::tool_execution(tool_name.clone(), e))?;
                serde_json::to_value(res)
                    .map_err(|e| BatchError::serialize_output(tool_name.clone(), e))
            },
            task_stop::NAME => {
                let args = serde_json::from_value::<task_stop::TaskStopArgs>(params)
                    .map_err(|e| BatchError::invalid_parameters(tool_name.clone(), e))?;
                let res = Tool::call(&task_stop::TaskStopTool::new(ctx), context, args)
                    .await
                    .map_err(|e| BatchError::tool_execution(tool_name.clone(), e))?;
                serde_json::to_value(res)
                    .map_err(|e| BatchError::serialize_output(tool_name.clone(), e))
            },
            webfetch::NAME => {
                let args = serde_json::from_value::<webfetch::WebFetchArgs>(params)
                    .map_err(|e| BatchError::invalid_parameters(tool_name.clone(), e))?;
                let res = Tool::call(&webfetch::WebFetchTool, context, args)
                    .await
                    .map_err(|e| BatchError::tool_execution(tool_name.clone(), e))?;
                serde_json::to_value(res)
                    .map_err(|e| BatchError::serialize_output(tool_name.clone(), e))
            },
            websearch::NAME => {
                let args = serde_json::from_value::<websearch::WebSearchArgs>(params)
                    .map_err(|e| BatchError::invalid_parameters(tool_name.clone(), e))?;
                let res = Tool::call(&websearch::WebSearchTool::new(), context, args)
                    .await
                    .map_err(|e| BatchError::tool_execution(tool_name.clone(), e))?;
                serde_json::to_value(res)
                    .map_err(|e| BatchError::serialize_output(tool_name.clone(), e))
            },
            codesearch::NAME => {
                let args = serde_json::from_value::<codesearch::CodeSearchArgs>(params)
                    .map_err(|e| BatchError::invalid_parameters(tool_name.clone(), e))?;
                let res = Tool::call(&codesearch::CodeSearchTool, context, args)
                    .await
                    .map_err(|e| BatchError::tool_execution(tool_name.clone(), e))?;
                serde_json::to_value(res)
                    .map_err(|e| BatchError::serialize_output(tool_name.clone(), e))
            },
            _ => Err(BatchError::unsupported_tool(tool_name)),
        }
    }
}

impl<C> Tool for BatchTool<C>
where
    C: AgentClientExt + Clone + Sync + Send + 'static,
{
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
        context: &mut rig::tool::ToolContext,
        args: Self::Args,
    ) -> Result<Self::Output, Self::Error> {
        let mut futures = Vec::new();

        for call in args.calls.into_iter().take(25) {
            let tool_name = call.tool.clone();
            let params = call.parameters;
            let mut context = context.clone();
            let future = async move {
                if tool_name == NAME {
                    Err(BatchError::nested_batch())
                } else {
                    self.dispatch(tool_name, params, &mut context).await
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
