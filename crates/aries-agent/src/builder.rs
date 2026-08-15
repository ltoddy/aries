use std::iter;
use std::path::{Path, PathBuf};

use aries_event::Notifier;
use aries_extension::AgentExtensions;
use aries_init::GlobalContext;
use aries_lspclient::SharedLspClient;
use aries_mode::Mode;
use aries_tools::agent;
use itertools::Itertools;
use rig_agent::client::AgentClientExt;
use rig_agent::tool::server::ToolServerHandle;

use crate::agent::{AGENT_LOOP_MAX_TURNS, AriesAgent};

pub struct AgentBuilder<C>
where
    C: AgentClientExt,
{
    client: C,
    model: String,
    mode: Mode,
    cwd: PathBuf,
    gctx: GlobalContext,
    lsp_client: Option<SharedLspClient>,

    extensions: AgentExtensions,

    notifier: Notifier,
}

impl<C> AgentBuilder<C>
where
    C: AgentClientExt + Clone + Send + Sync + 'static,
{
    pub fn new(
        client: C,
        model: impl Into<String>,
        mode: Mode,
        cwd: impl AsRef<Path>,
        gctx: GlobalContext,
        notifier: Notifier,
    ) -> Self {
        let cwd = cwd.as_ref().to_path_buf();
        let model = model.into();

        Self {
            client,
            model,
            mode,
            cwd,
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

    pub async fn build(
        self,
        tool_server_handle: ToolServerHandle,
    ) -> AriesAgent<C::CompletionModel> {
        let mode = self.mode;
        let name = mode.name();

        let mut tools = aries_tools::create_tools_from_mode(
            self.mode,
            &self.cwd,
            self.lsp_client.clone(),
            &self.extensions.skills,
        );
        if self.mode == Mode::Build || self.mode == Mode::General {
            tools.add_tool(agent::AgentTool::<C>::new(
                self.client.clone(),
                &self.model,
                &self.cwd,
                self.notifier.clone(),
                self.extensions.agents.clone(),
            ));
        }
        tool_server_handle.append_toolset(tools).await;

        let sections = aries_preamble::sections(
            self.gctx.clone(),
            &self.cwd,
            &self.model,
            &self.extensions.skills,
        );
        let preamble = iter::once(mode.bare_preamble().to_owned()).chain(sections).join("\n");

        let builder = self
            .client
            .agent(&self.model)
            .name(name)
            .description(mode.description())
            .preamble(&preamble)
            .tool_server_handle(tool_server_handle)
            .default_max_turns(AGENT_LOOP_MAX_TURNS);

        let inner = builder.build();

        AriesAgent::new(inner, name, preamble, self.notifier)
    }
}
