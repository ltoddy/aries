use std::path::PathBuf;

use anyhow::Result;
use futures::StreamExt;
use rig_core::agent::MultiTurnStreamItem;
use rig_core::client::CompletionClient;
use rig_core::completion::{Message, ToolDefinition};
use rig_core::tool::Tool;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::UnboundedSender;

use crate::agents::{AgentBuilder, AgentType};
use crate::error::AgentError;
use crate::event::AgentEvent;
use crate::tools::{RenderError, ToolArgsRender, ToolOutputRender};

pub const NAME: &str = "Agent";

#[derive(Debug, Deserialize, Serialize)]
pub struct AgentArgs {
    pub description: String,
    pub prompt: String,
    pub subagent_type: String,
    pub task_id: Option<String>,
}

impl AgentArgs {
    pub fn title(&self) -> String {
        format!("Launch {} subagent: {}", self.subagent_type, self.description)
    }
}

impl ToolArgsRender for AgentArgs {
    fn render_args(raw: &str) -> Result<(String, Option<String>), RenderError> {
        let args: Self = serde_json::from_str(raw)?;

        let mut first = args.description;
        first.push_str(&format!(", subagent_type = {}", args.subagent_type));
        if let Some(task_id) = &args.task_id {
            first.push_str(&format!(", task_id = {}", task_id));
        }

        Ok((first, Some(args.prompt)))
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AgentOutput {
    pub task_id: String,
    pub result: String,
}

impl ToolOutputRender for AgentOutput {
    fn render_output(raw: &str) -> Result<String, RenderError> {
        let output: Self = serde_json::from_str(raw)?;
        Ok(output.result)
    }
}

pub struct AgentTool<C>
where
    C: CompletionClient,
{
    client: C,
    model: String,
    cwd: PathBuf,
    sender: UnboundedSender<AgentEvent>,
}

impl<C> AgentTool<C>
where
    C: CompletionClient,
{
    pub fn new(
        client: C,
        model: impl Into<String>,
        cwd: PathBuf,
        sender: UnboundedSender<AgentEvent>,
    ) -> Self {
        let model = model.into();

        Self { client, model, cwd, sender }
    }
}

impl<C> Tool for AgentTool<C>
where
    C: CompletionClient + Clone + Send + Sync + 'static,
{
    const NAME: &'static str = NAME;
    type Error = AgentError;
    type Args = AgentArgs;
    type Output = AgentOutput;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: include_str!("descriptions/agent.txt").to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "description": {
                        "type": "string",
                        "description": "A short (3-5 words) description of the task"
                    },
                    "prompt": {
                        "type": "string",
                        "description": "The highly detailed task description for the agent to perform autonomously"
                    },
                    "subagent_type": {
                        "type": "string",
                        "description": "The type of agent to launch (e.g. 'explore', 'plan', 'default')"
                    },
                    "task_id": {
                        "type": "string",
                        "description": "Optional task ID to resume a previous subagent session"
                    }
                },
                "required": ["description", "prompt", "subagent_type"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let task_id = args.task_id.unwrap_or_else(|| nanoid::nanoid!());

        let agent_type = match args.subagent_type.as_str() {
            "default" | "build" => AgentType::Build,
            "explore" => AgentType::Explore,
            "plan" => AgentType::Plan,
            "general" => AgentType::General,
            _ => AgentType::General,
        };

        let client = self.client.clone();
        let model = self.model.clone();
        let cwd = self.cwd.clone();

        let mut agent = AgentBuilder::<C>::new(client, model, agent_type, cwd).build();

        let stream = agent.stream_prompt::<Vec<_>, Message>(&args.prompt, vec![]).await;
        tokio::pin!(stream);
        let mut final_res = rig_core::agent::FinalResponse::empty();

        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(item) => {
                    let event = AgentEvent::from_stream(false, agent_type.name(), item.clone());
                    let _ = self.sender.send(event);

                    if let MultiTurnStreamItem::FinalResponse(res) = item {
                        final_res = res;
                    }
                },
                Err(e) => {
                    return Err(AgentError::ExecutionError(format!("Subagent failed: {}", e)));
                },
            }
        }

        Ok(AgentOutput { task_id, result: final_res.response().to_owned() })
    }
}
