use std::iter;
use std::path::{Path, PathBuf};

use aries_event::Notifier;
use aries_extension::AgentExtensions;
use aries_init::GlobalContext;
use aries_lspclient::SharedLspClient;
use aries_mode::Mode;
use itertools::Itertools;
use rig::Model;
use rig::providers::openai::wire::OpenAiWire;
use rig::tool::server::ToolServerHandle;

use crate::agent::{AGENT_LOOP_MAX_TURNS, AriesAgent};

pub struct AgentBuilder {
    model: Model<OpenAiWire>,
    mode: Mode,
    root_dir: PathBuf,
    gctx: GlobalContext,
    lsp_client: Option<SharedLspClient>,

    extensions: AgentExtensions,

    notifier: Notifier,
}

impl AgentBuilder {
    pub fn new(
        model: Model<OpenAiWire>,
        mode: Mode,
        root_dir: impl AsRef<Path>,
        gctx: GlobalContext,
        notifier: Notifier,
    ) -> Self {
        let root_dir = root_dir.as_ref();

        Self {
            model,
            mode,
            root_dir: root_dir.to_owned(),
            gctx,
            lsp_client: None,
            extensions: AgentExtensions::empty(),
            notifier,
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

    pub async fn build(self, tool_server_handle: ToolServerHandle) -> AriesAgent {
        let mode = self.mode;
        let name = mode.name();
        let model_name = self.model.name();
        let current_dir = self.gctx.current_dir();

        let tools = aries_tools::create_tools_from_mode(
            self.mode,
            self.model.clone(),
            &current_dir,
            &self.root_dir,
            self.lsp_client.clone(),
            self.extensions.clone(),
            Notifier::clone(&self.notifier),
        );
        tool_server_handle.add_tools(tools);

        let sections =
            aries_preamble::sections(self.gctx, &current_dir, model_name, &self.extensions.skills)
                .await;
        let preamble = iter::once(mode.bare_preamble().to_owned()).chain(sections).join("\n");

        let builder = rig::agent::AgentBuilder::new(self.model)
            .name(name)
            .description(mode.description())
            .preamble(&preamble)
            .tool_server_handle(tool_server_handle)
            .default_max_turns(AGENT_LOOP_MAX_TURNS);

        let inner = builder.build();

        AriesAgent::new(inner, name, preamble, self.notifier)
    }
}
