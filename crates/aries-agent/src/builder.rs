use std::path::PathBuf;

use aries_event::AgentEvent;
use aries_extension::AgentExtensions;
use aries_lspclient::SharedLspClient;
use aries_mode::Mode;
use aries_tools::agent;
use rig_core::client::CompletionClient;
use rig_core::completion;
use rig_core::tool::ToolDyn;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::agent::{AGENT_LOOP_MAX_TURNS, AriesAgent};

pub struct AgentBuilder<C>
where
    C: CompletionClient,
    C::CompletionModel: completion::CompletionModel,
{
    client: C,
    model: String,
    mode: Mode,
    cwd: PathBuf,
    memory: Option<String>,
    lsp_client: Option<SharedLspClient>,
    use_tools: bool,

    extensions: AgentExtensions,
    mcp_tools: Vec<Box<dyn ToolDyn>>,

    sender: UnboundedSender<AgentEvent>,
    receiver: UnboundedReceiver<AgentEvent>,
}

impl<C> AgentBuilder<C>
where
    C: CompletionClient + Clone + Send + Sync + 'static,
    C::CompletionModel: completion::CompletionModel + 'static,
{
    pub fn new(client: C, model: impl Into<String>, mode: Mode, cwd: PathBuf) -> Self {
        let model = model.into();
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();

        Self {
            client,
            model,
            mode,
            cwd,
            memory: None,
            lsp_client: None,
            use_tools: true,
            extensions: AgentExtensions::empty(),
            mcp_tools: vec![],
            sender,
            receiver,
        }
    }

    pub fn with_memory(mut self, memory: Option<String>) -> Self {
        self.memory = memory;
        self
    }

    pub fn with_lsp_client(mut self, lsp_client: Option<SharedLspClient>) -> Self {
        self.lsp_client = lsp_client;
        self
    }

    pub fn with_use_tools(mut self, use_tools: bool) -> Self {
        self.use_tools = use_tools;
        self
    }

    pub fn with_extensions(mut self, extensions: AgentExtensions) -> Self {
        self.extensions = extensions;
        self
    }

    pub fn with_mcp_tools(mut self, tools: Vec<Box<dyn ToolDyn>>) -> Self {
        self.mcp_tools = tools;
        self
    }

    pub async fn build(self) -> (AriesAgent<C::CompletionModel>, UnboundedReceiver<AgentEvent>) {
        let mode = self.mode;
        let name = mode.name();

        let (preamble, tools) = if self.use_tools {
            let preamble = crate::preamble::render(
                &self.cwd,
                mode,
                &self.model,
                &self.extensions.skills,
                self.memory.as_deref(),
            )
            .await;

            let mut tools = self.build_tools();
            tools.extend(self.mcp_tools);

            (preamble, tools)
        } else {
            (mode.bare_preamble().to_owned(), vec![])
        };

        let inner = self
            .client
            .agent(self.model)
            .name(name)
            .description(mode.description())
            .preamble(&preamble)
            .tools(tools)
            .default_max_turns(AGENT_LOOP_MAX_TURNS)
            .build();

        (AriesAgent::new(inner, name, preamble, Some(self.sender)), self.receiver)
    }

    fn build_tools(&self) -> Vec<Box<dyn ToolDyn>> {
        let mut tools = aries_tools::create_tools_from_mode(
            self.mode,
            &self.cwd,
            self.lsp_client.clone(),
            &self.extensions.skills,
        );

        match self.mode {
            Mode::Build | Mode::General => {
                tools.push(Box::new(agent::AgentTool::<C>::new(
                    self.client.clone(),
                    self.model.clone(),
                    self.cwd.clone(),
                    self.sender.clone(),
                )));
            },
            _ => {},
        }

        tools
    }
}
