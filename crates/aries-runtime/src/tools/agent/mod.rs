mod args;
mod error;
mod output;

use aries_communicate::{Notifier, SubagentUpdate};
use aries_extension::AgentExtensions;
use aries_extension::agent::AgentDefinition;
use aries_init::GlobalContext;
use futures::StreamExt;
use rig::agent::{AgentBuilder, MultiTurnStreamItem, PromptResponse};
use rig::tool::server::ToolServer;
use rig::tool::{Tool, ToolContext};
use serde_json::Value;
use toasty::Db;

pub use self::args::AgentArgs;
pub use self::error::AgentError;
pub use self::output::AgentOutput;
use crate::context::ChatContext;
use crate::hooks::{TokenUsageHook, ToolCallHook, TranscriptHook};
use crate::preamble;
use crate::state::State;

pub const NAME: &str = "Agent";

const DESCRIPTION_HEAD: &str = include_str!("description-head.md");
const DESCRIPTION_TAIL: &str = include_str!("description-tail.md");

pub struct AgentTool {
    state: State,
    gctx: GlobalContext,
    ctx: crate::tools::context::ToolContext,
    notifier: Notifier,
    extensions: AgentExtensions,
    db: Db,

    agent_id_re: regex_lite::Regex,
}

impl AgentTool {
    pub fn new(
        state: State,
        ctx: crate::tools::context::ToolContext,
        gctx: GlobalContext,
        notifier: Notifier,
        extensions: AgentExtensions,
        db: Db,
    ) -> Self {
        let agent_id_re = regex_lite::Regex::new(r"\A[A-Za-z0-9_-]{21}\z")
            .expect("the hard-coded agent ID regex must be valid");

        Self { state, gctx, ctx, notifier, extensions, db, agent_id_re }
    }
}

impl Tool for AgentTool {
    const NAME: &'static str = NAME;
    type Args = AgentArgs;
    type Output = AgentOutput;
    type Error = AgentError;

    fn description(&self) -> String {
        let mut description = vec![DESCRIPTION_HEAD.to_owned()];

        if !self.extensions.agents.is_empty() {
            description.push("\n可用的自定义子智能体（把名字填入 `name`）：\n".to_owned());

            for AgentDefinition { location: _, frontmatter, body: _ } in &self.extensions.agents {
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
                "agent_id": {
                    "type": "string",
                    "description": "Optional agent ID returned by a previous Agent call in the current parent session. Provide it to reuse that agent's context and continue the task; omit it to create a new agent. Do not call the same agent ID concurrently."
                },
                "description": {
                    "type": "string",
                    "description": "Short user-facing title summarizing the delegated task."
                },
                "prompt": {
                    "type": "string",
                    "description": "For a new agent, provide a self-contained task brief: goal, relevant context and paths, constraints, whether edits are allowed, expected deliverable and validation. When reusing agent_id, provide follow-up instructions and any new information."
                },
                "name": {
                    "type": "string",
                    "description": "Name of an available agent configuration, matched case-insensitively. Determines instructions and available tools; unknown names return an error. Required even when reusing agent_id; use the same name to preserve the configuration."
                }
            },
            "required": ["description", "prompt", "name"]
        })
    }

    async fn call(
        &self,
        _context: &mut ToolContext,
        args: Self::Args,
    ) -> Result<Self::Output, Self::Error> {
        if let Some(ref agent_id) = args.agent_id
            && !self.agent_id_re.is_match(agent_id)
        {
            return Err(AgentError::invalid_agent_id(agent_id));
        }

        let Some(mode) = self
            .extensions
            .agents
            .iter()
            .find(|agent| agent.frontmatter.name.eq_ignore_ascii_case(&args.name))
        else {
            return Err(AgentError::agent_not_found(args.name));
        };

        let title = args.title();
        let agent_id = args.agent_id.unwrap_or(nanoid::nanoid!());
        let tool_names = mode.frontmatter.filter_tool_names(crate::tools::ALL_TOOL_NAMES);
        let cwd = self.ctx.cwd();
        let tools = crate::tools::create_agent_tools(
            self.state.clone(),
            &cwd,
            self.gctx.clone(),
            &tool_names,
            self.extensions.clone(),
            self.notifier.clone(),
            self.db.clone(),
        );

        let tool_server_handle = ToolServer::new().run();
        tool_server_handle.add_tools(tools);

        let token_usage_hook = TokenUsageHook::new(self.gctx.current_dir(), self.db.clone());
        let tool_call_hook = ToolCallHook::new(self.gctx.current_dir(), self.db.clone());
        let dir = self.state.session_dir().join("subagent").join(&agent_id);
        let transcript_hook = TranscriptHook::new(&dir)
            .await
            .map_err(|source| AgentError::context_persistence(&dir, source))?;

        let history = ChatContext::new(&dir)
            .await
            .map_err(|source| AgentError::context_persistence(&dir, source))?
            .history()
            .await;

        let agent = AgentBuilder::new(self.state.model())
            .name(&mode.frontmatter.name)
            .preamble(&mode.body)
            .append_preamble(&preamble::env::section(&cwd))
            .append_preamble(&preamble::skill::section(&self.extensions.skills))
            .append_preamble(&preamble::agentsmd::section(&cwd))
            .tool_server_handle(tool_server_handle)
            .add_hook(token_usage_hook)
            .add_hook(tool_call_hook)
            .add_hook(transcript_hook)
            .reasoning(self.state.effort())
            .default_max_turns(self.state.turns().as_usize())
            .build();

        let session_id = format!("agent-{}", nanoid::nanoid!());
        let subagent_update =
            SubagentUpdate::new(&session_id).title(title).description(args.description);
        self.notifier.send_subagent_update(subagent_update);

        let message_id = format!("agent-message-{}", nanoid::nanoid!());
        let mut stream = agent.prompt(args.prompt).history(history).stream();

        self.notifier.send_session_message(
            &message_id,
            self.state.session_id(),
            &session_id,
            &message_id,
            &message_id,
        );
        let mut final_res = PromptResponse::empty();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(AgentError::execution)?;
            self.notifier.send_session_message_chunk(&message_id, chunk.clone());

            if let MultiTurnStreamItem::FinalResponse(res) = chunk {
                final_res = res;
            }
        }

        Ok(AgentOutput::new(agent_id, final_res.output(), dir))
    }
}
