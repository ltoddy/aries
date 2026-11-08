use std::iter;
use std::path::{Path, PathBuf};

use aries_communicate::Notifier;
use aries_extension::AgentExtensions;
use aries_init::GlobalContext;
use itertools::Itertools;
use rig::Model;
use rig::agent::AgentHook;
use rig::providers::openai::wire::OpenAiWire;
use rig::tool::server::ToolServerHandle;
use toasty::Db;

use crate::agent::{AGENT_LOOP_MAX_TURNS, AriesAgent};
use crate::lspclient::SharedLspClient;

pub struct AgentBuilder {
    model: Model<OpenAiWire>,
    name: String,
    root_dir: PathBuf,
    cwd: PathBuf,
    gctx: GlobalContext,
    lsp_client: Option<SharedLspClient>,

    extensions: AgentExtensions,
    db: Db,

    notifier: Notifier,
    session_id: String,
}

impl AgentBuilder {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        session_id: impl Into<String>,
        model: Model<OpenAiWire>,
        name: impl Into<String>,
        root_dir: impl AsRef<Path>,
        cwd: impl AsRef<Path>,
        gctx: GlobalContext,
        notifier: Notifier,
        db: Db,
    ) -> Self {
        let root_dir = root_dir.as_ref();
        let cwd = cwd.as_ref();

        Self {
            model,
            name: name.into(),
            root_dir: root_dir.to_owned(),
            cwd: cwd.to_owned(),
            gctx,
            lsp_client: None,
            extensions: AgentExtensions::empty(),
            db,
            notifier,
            session_id: session_id.into(),
        }
    }

    pub fn with_lsp_client(mut self, lsp_client: Option<SharedLspClient>) -> Self {
        self.lsp_client = lsp_client;
        self
    }

    pub fn with_extensions(mut self, extensions: AgentExtensions) -> Self {
        self.extensions = extensions;
        self
    }

    pub async fn build<P>(self, tool_server_handle: ToolServerHandle, hook: P) -> AriesAgent
    where
        P: AgentHook + 'static,
    {
        let name = &self.name;
        let model_name = self.model.name();

        let tools = crate::create_tools(
            &self.session_id,
            self.gctx.clone(),
            crate::ALL_TOOL_NAMES,
            self.model.clone(),
            &self.root_dir,
            &self.cwd,
            self.lsp_client.clone(),
            self.extensions.clone(),
            Notifier::clone(&self.notifier),
            self.db.clone(),
        );
        tool_server_handle.add_tools(tools);

        let sections =
            crate::preamble::sections(self.gctx, &self.cwd, model_name, &self.extensions.skills)
                .await;
        let preamble =
            iter::once(String::from("You are a helpful AI assistant.")).chain(sections).join("\n");

        let builder = rig::agent::AgentBuilder::new(self.model)
            .name(name)
            .description(format!("{name} agent"))
            .preamble(&preamble)
            .tool_server_handle(tool_server_handle)
            .add_hook(hook)
            .default_max_turns(AGENT_LOOP_MAX_TURNS);

        let inner = builder.build();

        AriesAgent::new(inner, name.to_owned(), preamble, self.notifier)
    }
}
