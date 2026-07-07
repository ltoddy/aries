use std::path::PathBuf;

use aries_extension::mcp::{self, McpConfig};
use aries_extension::skill::definition::SkillDefinition;
use aries_extension::skill::loader::SkillsLoader;
use aries_lspclient::SharedLspClient;
use rig_core::client::CompletionClient;
use rig_core::completion;
use rig_core::tool::ToolDyn;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::agents::{AGENT_LOOP_MAX_TURNS, AriesAgent, Mode};
use crate::event::AgentEvent;
use crate::tools;

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

    mcp_config: McpConfig,

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
            mcp_config: McpConfig::empty(),
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

    pub fn with_mcp(mut self, mcp_config: McpConfig) -> Self {
        self.mcp_config = mcp_config;
        self
    }

    pub async fn build(self) -> (AriesAgent<C::CompletionModel>, UnboundedReceiver<AgentEvent>) {
        let mode = self.mode;
        let name = mode.name();

        let (preamble, mcp_clients, tools) = if self.use_tools {
            let skillloader = SkillsLoader::new(&self.cwd);
            let available_skills = skillloader.load().await.unwrap_or_default();

            let preamble = crate::preamble::render(
                &self.cwd,
                mode,
                &self.model,
                &available_skills,
                self.memory.as_deref(),
            )
            .await;

            let mut tools = self.build_tools(available_skills);

            let (mcp_clients, mcp_tools) = mcp::connect(self.mcp_config).await;
            tools.extend(mcp_tools);

            (preamble, Some(mcp_clients), tools)
        } else {
            (mode.bare_preamble().to_owned(), None, vec![])
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

        (AriesAgent::new(inner, name, preamble, Some(self.sender), mcp_clients), self.receiver)
    }

    fn build_tools(&self, available_skills: Vec<SkillDefinition>) -> Vec<Box<dyn ToolDyn>> {
        let mut names: Vec<&str> = vec![
            tools::bash::NAME,
            tools::read::NAME,
            tools::glob::NAME,
            tools::grep::NAME,
            tools::ls::NAME,
            tools::codesearch::NAME,
            tools::webfetch::NAME,
            tools::websearch::NAME,
        ];

        match self.mode {
            Mode::Build | Mode::General => {
                names.extend_from_slice(&[
                    tools::write::NAME,
                    tools::multiedit::NAME,
                    tools::edit::NAME,
                    tools::batch::NAME,
                    tools::update_plan::NAME,
                ]);
                names.push(tools::question::NAME);
                names.push(tools::skill::NAME);
                names.push(tools::lsp::NAME);
            },
            Mode::Plan => {
                names.push(tools::question::NAME);
            },
            Mode::Explore => {},
        }

        let mut tools = tools::create_tools(
            &names,
            &self.cwd,
            &self.sender,
            self.lsp_client.as_ref(),
            &available_skills,
        );

        match self.mode {
            Mode::Build | Mode::General => {
                tools.push(Box::new(tools::agent::AgentTool::<C>::new(
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
