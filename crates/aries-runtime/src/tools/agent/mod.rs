mod args;
mod error;
mod output;

use std::path::{Path, PathBuf};

use aries_communicate::{Notifier, SubagentUpdate};
use aries_extension::{AgentDefinition, AgentExtensions};
use aries_init::GlobalContext;
use futures::StreamExt;
use rig::Model;
use rig::agent::{MultiTurnStreamItem, PromptResponse};
use rig::providers::openai::wire::OpenAiWire;
use rig::tool::server::ToolServer;
use rig::tool::{Tool, ToolContext};
use serde_json::Value;
use toasty::Db;

pub use self::args::AgentArgs;
pub use self::error::AgentError;
pub use self::output::AgentOutput;
use crate::hooks::TokenUsageHook;

pub const NAME: &str = "Agent";
pub const DEFAULT_MAX_TURNS: usize = 100;

const DESCRIPTION_HEAD: &str = include_str!("description-head.md");
const DESCRIPTION_TAIL: &str = include_str!("description-tail.md");

pub struct AgentTool {
    gctx: GlobalContext,
    ctx: crate::tools::ToolContext,
    model: Model<OpenAiWire>,
    parent_dir: PathBuf,
    notifier: Notifier,
    extensions: AgentExtensions,
    hook: TokenUsageHook,
    db: Db,
}

impl AgentTool {
    pub fn new(
        gctx: GlobalContext,
        ctx: crate::tools::ToolContext,
        model: Model<OpenAiWire>,
        parent_dir: impl AsRef<Path>,
        notifier: Notifier,
        extensions: AgentExtensions,
        db: Db,
    ) -> Self {
        let parent_dir = parent_dir.as_ref();
        let hook = TokenUsageHook::new(gctx.current_dir(), db.clone());

        Self { gctx, ctx, model, parent_dir: parent_dir.to_owned(), notifier, extensions, hook, db }
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
                "description": {
                    "type": "string",
                    "description": "Short user-facing title summarizing the delegated task."
                },
                "prompt": {
                    "type": "string",
                    "description": "Self-contained task brief: goal, relevant context and paths, constraints, whether edits are allowed, expected deliverable and validation."
                },
                "name": {
                    "type": "string",
                    "description": "Built-in build/general allow shell commands and file edits; explore is read-only research; plan is read-only analysis and planning. Also accepts listed custom names, matched case-insensitively before built-ins. Unknown values fall back to general."
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
        let task_id = nanoid::nanoid!();

        let Some(extension) = self
            .extensions
            .agents
            .iter()
            .find(|agent| agent.frontmatter.name.eq_ignore_ascii_case(&args.name))
        else {
            return Err(AgentError::AgentNotFound(args.name));
        };

        let tool_names = extension.frontmatter.filter_tool_names(crate::ALL_TOOL_NAMES);
        let cwd = self.ctx.cwd();
        let tools = crate::create_tools(
            self.ctx.session_id(),
            self.gctx.clone(),
            &tool_names,
            self.model.clone(),
            &self.parent_dir,
            &cwd,
            AgentExtensions::empty(),
            Notifier::clone(&self.notifier),
            self.db.clone(),
        );

        let tool_server_handle = ToolServer::new().run();
        tool_server_handle.add_tools(tools);

        let cwd = self
            .ctx
            .worktree()
            .map_or_else(|| self.ctx.cwd(), |worktree| worktree.path().to_owned());
        let agent = rig::agent::AgentBuilder::new(self.model.clone())
            .name(&extension.frontmatter.name)
            .preamble(&extension.body)
            .append_preamble(&crate::preamble::env::section(cwd, self.model.name()))
            .tool_server_handle(tool_server_handle)
            .add_hook(self.hook.clone())
            .default_max_turns(DEFAULT_MAX_TURNS)
            .build();

        let session_id = format!("agent-{}", nanoid::nanoid!());
        let subagent_update =
            SubagentUpdate::new(&session_id).title(args.title()).description(args.description);
        self.notifier.send_subagent_update(subagent_update);

        let message_id = format!("agent-message-{}", nanoid::nanoid!());
        let mut stream = agent.prompt(args.prompt).stream();

        self.notifier.send_session_message(
            &message_id,
            self.ctx.session_id(),
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

        let mut file_path = self.parent_dir.join("subagent").join(&task_id);
        let messages = final_res.messages();
        if let Some(parent) = file_path.parent() {
            _ = tokio::fs::create_dir_all(parent).await;
        }
        file_path.set_extension("jsonl");
        let _ = aries_filesystem::jsonl::write(&file_path, messages).await;

        Ok(AgentOutput::new(task_id, final_res.output(), file_path))
    }
}
