use std::path::PathBuf;

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

    pub async fn build(self) -> (AriesAgent<C::CompletionModel>, UnboundedReceiver<AgentEvent>) {
        let mode = self.mode;
        let name = mode.name();

        let (preamble, tools) = if self.use_tools {
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

            let tools = self.build_tools(available_skills);
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

    fn build_tools(&self, available_skills: Vec<SkillDefinition>) -> Vec<Box<dyn ToolDyn>> {
        let mode = self.mode;
        let client = self.client.clone();
        let cwd = self.cwd.clone();
        let model = self.model.clone();

        let mut tools: Vec<Box<dyn ToolDyn>> = vec![
            Box::new(tools::bash::BashTool),
            Box::new(tools::read::ReadTool),
            Box::new(tools::glob::GlobTool::new(self.cwd.clone())),
            Box::new(tools::grep::GrepTool::new(self.cwd.clone())),
            Box::new(tools::ls::LsTool::new(self.cwd.clone())),
            Box::new(tools::codesearch::CodeSearchTool),
            Box::new(tools::webfetch::WebFetchTool),
            Box::new(tools::websearch::WebSearchTool),
        ];

        if matches!(mode, Mode::Build | Mode::General) {
            tools.push(Box::new(tools::write::WriteTool));
            tools.push(Box::new(tools::multiedit::MultiEditTool));
            tools.push(Box::new(tools::edit::EditTool));
            tools.push(Box::new(tools::batch::BatchTool::new(self.cwd.clone())));
            tools.push(Box::new(tools::update_plan::UpdatePlanTool::new(self.sender.clone())));
            tools.push(Box::new(tools::agent::AgentTool::<C>::new(
                client,
                model,
                cwd,
                self.sender.clone(),
            )));
        }

        if matches!(mode, Mode::Build | Mode::General | Mode::Plan) {
            tools.push(Box::new(tools::question::AskUserQuestionTool));
        }

        if matches!(mode, Mode::Build | Mode::General) {
            if !available_skills.is_empty() {
                tools.push(Box::new(tools::skill::SkillTool::new(available_skills)));
            }
            if let Some(ref lsp_client) = self.lsp_client {
                tools
                    .push(Box::new(tools::lsp::LspTool::new(lsp_client.clone(), self.cwd.clone())));
            }
        }

        tools
    }
}
