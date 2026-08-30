mod args;
mod output;

use std::path::{Path, PathBuf};

use aries_event::Notifier;
use aries_extension::{AgentDefinition, AgentExtensions};
use aries_mode::Mode;
use futures::StreamExt;
use rig::agent::{MultiTurnStreamItem, PromptResponse, StreamingError};
use rig::client::AgentClientExt;
use rig::streaming::StreamingPrompt;
use rig::tool::server::ToolServer;
use rig::tool::{Tool, ToolContext};
use serde_json::Value;

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
    notifier: Notifier,
    extensions: AgentExtensions,
}

impl<C> AgentTool<C>
where
    C: AgentClientExt,
{
    pub fn new(
        client: C,
        model: impl Into<String>,
        cwd: impl AsRef<Path>,
        notifier: Notifier,
        extensions: AgentExtensions,
    ) -> Self {
        let model = model.into();
        let cwd = cwd.as_ref().to_owned();

        Self { client, model, cwd, notifier, extensions }
    }

    fn find_agent(&self, mode: impl Into<String>) -> Option<&AgentDefinition> {
        let mode = mode.into();

        self.extensions
            .agents
            .iter()
            .find(|agent| agent.frontmatter.name.eq_ignore_ascii_case(&mode))
    }
}

impl<C> Tool for AgentTool<C>
where
    C: AgentClientExt + Clone + Send + Sync + 'static,
{
    const NAME: &'static str = NAME;
    type Args = AgentArgs;
    type Output = AgentOutput;
    type Error = StreamingError;

    fn description(&self) -> String {
        let mut description = vec![DESCRIPTION_HEAD.to_owned()];

        if !self.extensions.agents.is_empty() {
            description.push("\n可用的自定义子智能体（把名字填入 `mode`）：\n".to_owned());

            for AgentDefinition { frontmatter, .. } in &self.extensions.agents {
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
                let tools = create_tools_from_tool_names(
                    &tool_names,
                    self.client.clone(),
                    &self.model,
                    &self.cwd,
                    None,
                    AgentExtensions::empty(),
                    Notifier::clone(&self.notifier),
                );
                let model = frontmatter.model.clone().unwrap_or_else(|| self.model.clone());
                (name, preamble, tools, model)
            },
            None => {
                let mode = args.mode.parse::<Mode>().unwrap_or(Mode::General);
                (
                    mode.name().to_owned(),
                    mode.bare_preamble().to_owned(),
                    create_tools_from_mode(
                        mode,
                        self.client.clone(),
                        &self.model,
                        &self.cwd,
                        None,
                        AgentExtensions::empty(),
                        Notifier::clone(&self.notifier),
                    ),
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
            self.notifier.send_stream_item(chunk.clone());

            if let MultiTurnStreamItem::FinalResponse(res) = chunk {
                final_res = res;
            }
        }

        Ok(AgentOutput { task_id: args.task_id, result: final_res.output })
    }
}
