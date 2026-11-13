use aries_communicate::Notifier;
use aries_extension::AgentExtensions;
use aries_filesystem::path_to_slug;
use aries_init::GlobalContext;
use rig::agent::{AgentBuilder, AgentHook};
use rig::tool::server::ToolServerHandle;
use toasty::Db;

use crate::agent::AriesAgent;
use crate::hooks::{TokenUsageHook, ToolCallHook};
use crate::preamble;
use crate::state::State;

pub struct Builder {
    state: State,
    extensions: AgentExtensions,
    gctx: GlobalContext,
    tool_server_handle: ToolServerHandle,

    db: Db,
    notifier: Notifier,
}

impl Builder {
    pub fn new(
        state: State,
        extensions: AgentExtensions,
        tool_server_handle: ToolServerHandle,
        gctx: GlobalContext,
        notifier: Notifier,
        db: Db,
    ) -> Self {
        Self { state, extensions, gctx, tool_server_handle, db, notifier }
    }

    pub async fn build<P>(self, hook: P) -> AriesAgent
    where
        P: AgentHook + 'static,
    {
        let tools = crate::tools::create_tools(
            self.state.clone(),
            self.gctx.clone(),
            crate::tools::ALL_TOOL_NAMES,
            self.extensions.clone(),
            self.notifier.clone(),
            self.db.clone(),
        );
        self.tool_server_handle.add_tools(tools);

        let cwd = &self.state.cwd;
        let token_usage_hook = TokenUsageHook::new(cwd, self.db.clone());
        let tool_call_hook = ToolCallHook::new(cwd, self.db.clone());
        let builder = AgentBuilder::new(self.state.model)
            .name(&self.state.agent.frontmatter.name)
            .description(&self.state.agent.frontmatter.description)
            .preamble(&self.state.agent.body)
            .append_preamble(&preamble::skill::section(&self.extensions.skills))
            .append_preamble(&preamble::env::section(cwd))
            .append_preamble(&preamble::memory::section(
                self.gctx.memory_root_dir().join(path_to_slug(cwd)),
            ))
            .append_preamble(&preamble::agentsmd::section(cwd))
            .tool_server_handle(self.tool_server_handle)
            .add_hook(token_usage_hook)
            .add_hook(tool_call_hook)
            .add_hook(hook)
            .reasoning(self.state.effort)
            .default_max_turns(self.state.turns.as_usize());

        let inner = builder.build();

        AriesAgent::new(inner, self.state.agent.body, self.notifier)
    }
}
