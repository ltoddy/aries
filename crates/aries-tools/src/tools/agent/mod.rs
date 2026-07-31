mod args;
mod output;

use std::path::{Path, PathBuf};

use aries_event::AgentEvent;
use aries_extension::agent::AgentDefinition;
use aries_mode::Mode;
use futures::StreamExt;
use rig_agent::agent::{MultiTurnStreamItem, PromptResponse, StreamingError};
use rig_agent::client::AgentClientExt;
use rig_agent::streaming::StreamingPrompt;
use rig_agent::tool::server::ToolServer;
use rig_agent::tool::{Tool, ToolContext};
use serde_json::Value;
use tokio::sync::mpsc::UnboundedSender;

pub use self::args::AgentArgs;
pub use self::output::AgentOutput;
use crate::{create_tools_from_mode, create_tools_from_tool_names, tool_names_from_mode};

pub const NAME: &str = "Agent";
pub const DEFAULT_MAX_TURNS: usize = 100;

const DESCRIPTION_HEAD: &str = include_str!("description-head.md");
const DESCRIPTION_TAIL: &str = include_str!("description-tail.md");

pub struct AgentTool<C>
where
    C: AgentClientExt,
{
    client: C,
    model: String,
    cwd: PathBuf,
    sender: UnboundedSender<AgentEvent>,
    custom_agents: Vec<AgentDefinition>,
}

impl<C> AgentTool<C>
where
    C: AgentClientExt,
{
    pub fn new(
        client: C,
        model: impl Into<String>,
        cwd: impl AsRef<Path>,
        sender: UnboundedSender<AgentEvent>,
        custom_agents: Vec<AgentDefinition>,
    ) -> Self {
        let model = model.into();
        let cwd = cwd.as_ref().to_path_buf();

        Self { client, model, cwd, sender, custom_agents }
    }

    fn find_agent(&self, mode: impl Into<String>) -> Option<&AgentDefinition> {
        let mode = mode.into();

        self.custom_agents.iter().find(|agent| agent.frontmatter.name.eq_ignore_ascii_case(&mode))
    }
}

impl<C> Tool for AgentTool<C>
where
    C: AgentClientExt + Clone + Send + Sync + 'static,
{
    const NAME: &'static str = NAME;
    type Error = StreamingError;
    type Args = AgentArgs;
    type Output = AgentOutput;

    fn description(&self) -> String {
        let mut description = vec![DESCRIPTION_HEAD.to_owned()];

        if !self.custom_agents.is_empty() {
            description.push("\n可用的自定义子智能体（把名字填入 `mode`）：\n".to_owned());

            for AgentDefinition { frontmatter, .. } in &self.custom_agents {
                let desc = format!(
                    "- {}: {} (Tools: {})\n",
                    frontmatter.name,
                    frontmatter.description,
                    frontmatter.tools_description(),
                );
                description.push(desc);
            }
        }
        description.push(DESCRIPTION_TAIL.to_owned());

        description.join("\n")
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "description": {
                    "type": "string",
                    "description": "A short description of the task"
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

    async fn call(
        &self,
        _context: &mut ToolContext,
        args: Self::Args,
    ) -> Result<Self::Output, Self::Error> {
        let (name, preamble, tools, model) = match self.find_agent(&args.mode) {
            Some(AgentDefinition { frontmatter, body, .. }) => {
                let name = frontmatter.name.clone();
                let preamble = body.clone();
                let universe = tool_names_from_mode(Mode::General);
                let tool_names = frontmatter.filter_tool_names(&universe);
                let tools = create_tools_from_tool_names(&tool_names, &self.cwd, None, &[]);
                let model = frontmatter.model.clone().unwrap_or_else(|| self.model.clone());
                (name, preamble, tools, model)
            },
            None => {
                let mode = args.mode.parse::<Mode>().unwrap_or(Mode::General);
                (
                    mode.name().to_owned(),
                    mode.bare_preamble().to_owned(),
                    create_tools_from_mode(mode, &self.cwd, None, &[]),
                    self.model.clone(),
                )
            },
        };

        let tool_server_handle = ToolServer::new().run();
        tool_server_handle.append_toolset(tools).await;

        let agent = self
            .client
            .agent(&model)
            .name(&name)
            .preamble(&preamble)
            .append_preamble(&aries_preamble::env::section(&self.cwd, &model))
            .tool_server_handle(tool_server_handle)
            .default_max_turns(DEFAULT_MAX_TURNS)
            .build();

        let mut stream = agent.stream_prompt(args.prompt).await;

        let mut final_res = PromptResponse::empty();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            let event = AgentEvent::from_stream(false, &name, chunk.clone());
            let _ = self.sender.send(event);

            if let MultiTurnStreamItem::FinalResponse(res) = chunk {
                final_res = res;
            }
        }

        Ok(AgentOutput { task_id: args.task_id, result: final_res.output })
    }
}
