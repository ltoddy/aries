use std::path::{Path, PathBuf};

use aries_event::AgentEvent;
use aries_extension::AgentExtensions;
use aries_init::GlobalContext;
use aries_lspclient::SharedLspClient;
use aries_mode::Mode;
use aries_tools::agent;
use rig_agent::client::AgentClientExt;
use rig_agent::completion;
use rig_agent::tool::ToolSet;
use rig_agent::tool::server::ToolServerHandle;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::agent::{AGENT_LOOP_MAX_TURNS, AriesAgent};

pub struct AgentBuilder<C>
where
    C: AgentClientExt,
    C::CompletionModel: completion::CompletionModel,
{
    client: C,
    model: String,
    mode: Mode,
    cwd: PathBuf,
    gctx: GlobalContext,
    lsp_client: Option<SharedLspClient>,

    extensions: AgentExtensions,

    sender: UnboundedSender<AgentEvent>,
    receiver: UnboundedReceiver<AgentEvent>,
}

impl<C> AgentBuilder<C>
where
    C: AgentClientExt + Clone + Send + Sync + 'static,
    C::CompletionModel: completion::CompletionModel + 'static,
{
    pub fn new(
        client: C,
        model: impl Into<String>,
        mode: Mode,
        cwd: impl AsRef<Path>,
        gctx: GlobalContext,
    ) -> Self {
        let cwd = cwd.as_ref().to_path_buf();
        let model = model.into();
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();

        Self {
            client,
            model,
            mode,
            cwd,
            gctx,
            lsp_client: None,
            extensions: AgentExtensions::empty(),
            sender,
            receiver,
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
    ) -> (AriesAgent<C::CompletionModel>, UnboundedReceiver<AgentEvent>) {
        let mode = self.mode;
        let name = mode.name();

        let tools = self.build_tools();
        tool_server_handle.append_toolset(tools).await;

        let sections = aries_preamble::sections(
            self.gctx.clone(),
            &self.cwd,
            &self.model,
            &self.extensions.skills,
        );

        let mut builder = self
            .client
            .agent(&self.model)
            .tool_server_handle(tool_server_handle)
            .default_max_turns(AGENT_LOOP_MAX_TURNS)
            .name(name)
            .description(mode.description())
            .preamble(mode.bare_preamble());

        for section in sections.iter() {
            builder = builder.append_preamble(section);
        }

        let inner = builder.build();

        (
            AriesAgent::new(inner, name, mode.bare_preamble(), &sections, Some(self.sender)),
            self.receiver,
        )
    }

    fn build_tools(&self) -> ToolSet {
        let mut tools = aries_tools::create_tools_from_mode(
            self.mode,
            &self.cwd,
            self.lsp_client.clone(),
            &self.extensions.skills,
        );

        match self.mode {
            Mode::Build | Mode::General => {
                tools.add_tool(agent::AgentTool::<C>::new(
                    self.client.clone(),
                    self.model.clone(),
                    self.cwd.clone(),
                    self.sender.clone(),
                    self.extensions.agents.clone(),
                ));
            },
            _ => {},
        }

        tools
    }
}
