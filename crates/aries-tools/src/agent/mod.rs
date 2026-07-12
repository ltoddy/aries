mod args;
mod output;

use std::path::{Path, PathBuf};

use aries_event::AgentEvent;
use aries_mode::Mode;
use futures::StreamExt;
use rig_core::agent::{MultiTurnStreamItem, StreamingError};
use rig_core::client::CompletionClient;
use rig_core::streaming::StreamingPrompt;
use rig_core::tool::Tool;
use serde_json::Value;
use tokio::sync::mpsc::UnboundedSender;

pub use self::args::AgentArgs;
pub use self::output::AgentOutput;
use crate::create_tools_from_mode;

pub const NAME: &str = "Agent";
const DEFAULT_MAX_TURNS: usize = 100;

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
        cwd: impl AsRef<Path>,
        sender: UnboundedSender<AgentEvent>,
    ) -> Self {
        let model = model.into();
        let cwd = cwd.as_ref().to_path_buf();

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

    fn description(&self) -> String {
        include_str!("description.md").to_owned()
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
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
        })
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let mode = args.mode.parse::<Mode>().unwrap_or(Mode::General);

        let agent = self
            .client
            .agent(&self.model)
            .name(mode.name())
            .description(mode.description())
            .preamble(mode.bare_preamble())
            .tools(create_tools_from_mode(mode, &self.cwd, None, &[]))
            .default_max_turns(DEFAULT_MAX_TURNS)
            .build();

        let mut stream = agent.stream_prompt(args.prompt).await;

        let mut final_res = rig_core::agent::PromptResponse::empty();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            let event = AgentEvent::from_stream(false, mode.name(), chunk.clone());
            let _ = self.sender.send(event);

            if let MultiTurnStreamItem::FinalResponse(res) = chunk {
                final_res = res;
            }
        }

        Ok(AgentOutput { task_id: args.task_id, result: final_res.output })
    }
}
