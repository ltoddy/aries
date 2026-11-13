use aries_communicate::Notifier;
use aries_extension::AgentExtensions;
use aries_init::GlobalContext;
use rig::agent::AgentHook;
use rig::tool::server::ToolServerHandle;
use toasty::Db;

use crate::agent::AriesAgent;
use crate::state::State;

pub struct AgentBuilder {
    state: State,
    extensions: AgentExtensions,
    gctx: GlobalContext,

    db: Db,
    notifier: Notifier,
}

impl AgentBuilder {
    pub fn new(
        state: State,
        extensions: AgentExtensions,
        gctx: GlobalContext,
        notifier: Notifier,
        db: Db,
    ) -> Self {
        Self { state, gctx, extensions, db, notifier }
    }

    pub async fn build<P>(self, tool_server_handle: ToolServerHandle, hook: P) -> AriesAgent
    where
        P: AgentHook + 'static,
    {
        let model = self.state.model.name();
        let sections =
            crate::preamble::sections(&self.gctx, &self.state.cwd, model, &self.extensions.skills)
                .await;

        let tools = crate::create_tools(
            self.state.clone(),
            self.gctx,
            crate::ALL_TOOL_NAMES,
            self.extensions,
            self.notifier.clone(),
            self.db.clone(),
        );
        tool_server_handle.add_tools(tools);

        let mut builder = rig::agent::AgentBuilder::new(self.state.model)
            .name(&self.state.agent.frontmatter.name)
            .description(&self.state.agent.frontmatter.description)
            .preamble(&self.state.agent.body)
            .tool_server_handle(tool_server_handle)
            .add_hook(hook)
            .reasoning(self.state.effort)
            .default_max_turns(self.state.turns.as_usize());
        for section in sections {
            builder = builder.append_preamble(&section);
        }

        let inner = builder.build();

        AriesAgent::new(inner, self.state.agent.body, self.notifier)
    }
}
