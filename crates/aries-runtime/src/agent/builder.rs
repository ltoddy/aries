use std::path::{Path, PathBuf};

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

    root_dir: PathBuf,
    cwd: PathBuf,
    gctx: GlobalContext,

    extensions: AgentExtensions,
    db: Db,

    notifier: Notifier,
    session_id: String,
}

impl AgentBuilder {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        session_id: impl Into<String>,
        state: State,
        root_dir: impl AsRef<Path>,
        cwd: impl AsRef<Path>,
        gctx: GlobalContext,
        notifier: Notifier,
        db: Db,
    ) -> Self {
        let root_dir = root_dir.as_ref();
        let cwd = cwd.as_ref();

        Self {
            state,
            root_dir: root_dir.to_owned(),
            cwd: cwd.to_owned(),
            gctx,
            extensions: AgentExtensions::empty(),
            db,
            notifier,
            session_id: session_id.into(),
        }
    }

    pub fn with_extensions(mut self, extensions: AgentExtensions) -> Self {
        self.extensions = extensions;
        self
    }

    pub async fn build<P>(self, tool_server_handle: ToolServerHandle, hook: P) -> AriesAgent
    where
        P: AgentHook + 'static,
    {
        let model = self.state.model.name();

        let tools = crate::create_tools(
            &self.session_id,
            self.gctx.clone(),
            crate::ALL_TOOL_NAMES,
            self.state.model.clone(),
            &self.root_dir,
            &self.cwd,
            self.extensions.clone(),
            Notifier::clone(&self.notifier),
            self.db.clone(),
        );
        tool_server_handle.add_tools(tools);

        let sections =
            crate::preamble::sections(self.gctx, &self.cwd, model, &self.extensions.skills).await;

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
