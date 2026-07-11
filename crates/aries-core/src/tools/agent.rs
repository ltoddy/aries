use std::path::PathBuf;

pub use aries_tools::agent::{AgentArgs, AgentOutput, NAME};
use futures::StreamExt;
use rig_core::agent::{MultiTurnStreamItem, StreamingError};
use rig_core::client::CompletionClient;
use rig_core::completion::{Message, ToolDefinition};
use rig_core::tool::Tool;
use tokio::sync::mpsc::UnboundedSender;

use crate::agents::{AgentBuilder, Mode};
use crate::event::AgentEvent;

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
    type Error = StreamingError;
    type Args = AgentArgs;
    type Output = AgentOutput;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_owned(),
            description: include_str!("agent.md").to_owned(),
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
                    "mode": {
                        "type": "string",
                        "description": "The type of agent to launch (e.g. 'explore', 'plan', 'default')"
                    },
                    "task_id": {
                        "type": "string",
                        "description": "Optional task ID to resume a previous subagent session"
                    }
                },
                "required": ["description", "prompt", "mode"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let task_id = args.task_id.unwrap_or_else(|| nanoid::nanoid!());

        let mode = match args.mode.as_str() {
            "default" | "build" => Mode::Build,
            "explore" => Mode::Explore,
            "plan" => Mode::Plan,
            "general" => Mode::General,
            _ => Mode::General,
        };

        let client = self.client.clone();
        let model = self.model.clone();
        let cwd = self.cwd.clone();

        let (mut agent, _receiver) =
            AgentBuilder::new(client, model, mode, cwd).with_use_tools(false).build().await;

        let stream = agent.stream_prompt::<Vec<_>, Message>(&args.prompt, vec![]).await;
        tokio::pin!(stream);
        let mut final_res = rig_core::agent::FinalResponse::empty();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            let event = AgentEvent::from_stream(false, mode.name(), chunk.clone());
            let _ = self.sender.send(event);

            if let MultiTurnStreamItem::FinalResponse(res) = chunk {
                final_res = res;
            }
        }

        Ok(AgentOutput { task_id, result: final_res.response().to_owned() })
    }
}
