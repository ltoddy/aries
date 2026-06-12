use std::path::PathBuf;

use rig_core::client::CompletionClient;
use rig_core::completion;
use rig_core::tool::ToolDyn;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::agents::{AGENT_LOOP_MAX_TURNS, AgentType, AriesAgent};
use crate::event::AgentEvent;
use crate::ext::skill::{SkillDefinition, SkillsLoader};
use crate::language_server::SharedLspClient;
use crate::tools;

pub struct AgentBuilder<C>
where
    C: CompletionClient,
    C::CompletionModel: completion::CompletionModel,
{
    client: C,
    model: String,
    agent_type: AgentType,
    cwd: PathBuf,

    sender: UnboundedSender<AgentEvent>,
    receiver: UnboundedReceiver<AgentEvent>,
}

impl<C> AgentBuilder<C>
where
    C: CompletionClient + Clone + Send + Sync + 'static,
    C::CompletionModel: completion::CompletionModel + 'static,
{
    pub fn new(client: C, model: impl Into<String>, agent_type: AgentType, cwd: PathBuf) -> Self {
        let model = model.into();
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();

        Self { client, model, agent_type, cwd, sender, receiver }
    }

    pub fn build(self) -> AriesAgent<C::CompletionModel> {
        let agent_type = self.agent_type;

        let name = agent_type.name();
        let preamble = agent_type.bare_preamble().to_owned();

        let inner = self
            .client
            .agent(self.model)
            .name(name)
            .description(agent_type.description())
            .preamble(&preamble)
            .default_max_turns(AGENT_LOOP_MAX_TURNS)
            .build();

        AriesAgent::new(inner, name, preamble, Some(self.sender))
    }

    pub async fn with_tools(
        self,
        lsp_client: Option<SharedLspClient>,
    ) -> (AriesAgent<C::CompletionModel>, UnboundedReceiver<AgentEvent>) {
        let agent_type = self.agent_type;

        let skillloader = SkillsLoader::new(&self.cwd);
        let available_skills = skillloader.load().await.unwrap_or_default();

        let name = agent_type.name();
        let preamble =
            crate::preamble::render(&self.cwd, agent_type, &self.model, &available_skills).await;

        let tools = self.build_tools(lsp_client, available_skills);

        let inner = self
            .client
            .agent(self.model)
            .name(name)
            .description(agent_type.description())
            .preamble(&preamble)
            .tools(tools)
            .default_max_turns(AGENT_LOOP_MAX_TURNS)
            .build();

        (AriesAgent::new(inner, name, preamble, Some(self.sender)), self.receiver)
    }

    fn build_tools(
        &self,
        lsp_client: Option<SharedLspClient>,
        available_skills: Vec<SkillDefinition>,
    ) -> Vec<Box<dyn ToolDyn>> {
        let agent_type = self.agent_type;
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

        if matches!(agent_type, AgentType::Build | AgentType::General) {
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

        if matches!(agent_type, AgentType::Build | AgentType::General | AgentType::Plan) {
            tools.push(Box::new(tools::question::AskUserQuestionTool));
        }

        if matches!(agent_type, AgentType::Build | AgentType::General) {
            if !available_skills.is_empty() {
                tools.push(Box::new(tools::skill::SkillTool::new(available_skills)));
            }
            if let Some(lsp_client) = lsp_client {
                tools.push(Box::new(tools::lsp::LspTool::new(lsp_client, self.cwd.clone())));
            }
        }

        tools
    }
}
