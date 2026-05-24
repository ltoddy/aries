use std::path::PathBuf;

use aries_config::AriesConfig;
use rig::agent::PromptHook;
use rig::client::CompletionClient;
use rig::completion;
use rig::tool::ToolDyn;

use crate::agents::{AGENT_LOOP_MAX_TURNS, AgentType, AriesAgent};
use crate::ext::skill::{SkillDefinition, SkillsLoader};
use crate::language_server::SharedLspClient;
use crate::tools;

pub struct AgentBuilder<C, P = ()>
where
    C: CompletionClient,
    C::CompletionModel: completion::CompletionModel,
{
    client: C,
    config: AriesConfig,
    agent_type: AgentType,
    cwd: PathBuf,
    hook: P,
}

impl<C, P> AgentBuilder<C, P>
where
    C: CompletionClient + Clone + Send + Sync + 'static,
    C::CompletionModel: completion::CompletionModel + 'static,
    P: PromptHook<C::CompletionModel>,
{
    pub fn new(
        client: C,
        config: AriesConfig,
        agent_type: AgentType,
        cwd: PathBuf,
        hook: P,
    ) -> Self {
        Self { client, config, agent_type, cwd, hook }
    }

    pub fn build(self) -> AriesAgent<C::CompletionModel, P> {
        let model = self.config.model().to_owned();
        let agent_type = self.agent_type;

        let name = agent_type.name();
        let preamble = agent_type.bare_preamble().to_owned();

        let inner = self
            .client
            .agent(model)
            .hook(self.hook)
            .name(name)
            .description(agent_type.description())
            .preamble(&preamble)
            .default_max_turns(AGENT_LOOP_MAX_TURNS)
            .build();

        AriesAgent::new(inner, name, preamble)
    }

    pub async fn with_tools(
        self,
        lsp_client: Option<SharedLspClient>,
    ) -> AriesAgent<C::CompletionModel, P> {
        let model = self.config.model().to_owned();
        let agent_type = self.agent_type;

        let skillloader = SkillsLoader::new(&self.cwd);
        let available_skills = skillloader.load().await.unwrap_or_default();

        let name = agent_type.name();
        let preamble =
            crate::preamble::render(&self.cwd, agent_type, &model, &available_skills).await;

        let tools = self.build_tools(lsp_client, available_skills);

        let inner = self
            .client
            .agent(model)
            .hook(self.hook)
            .name(name)
            .description(agent_type.description())
            .preamble(&preamble)
            .tools(tools)
            .default_max_turns(AGENT_LOOP_MAX_TURNS)
            .build();

        AriesAgent::new(inner, name, preamble)
    }

    fn build_tools(
        &self,
        lsp_client: Option<SharedLspClient>,
        available_skills: Vec<SkillDefinition>,
    ) -> Vec<Box<dyn ToolDyn>> {
        let agent_type = self.agent_type;
        let config = self.config.clone();
        let client = self.client.clone();

        let mut tools: Vec<Box<dyn ToolDyn>> = vec![
            Box::new(tools::bash::BashTool),
            Box::new(tools::read::ReadTool),
            Box::new(tools::glob::GlobTool::new(self.cwd.clone())),
            Box::new(tools::grep::GrepTool::new(self.cwd.clone())),
            Box::new(tools::ls::LsTool::new(self.cwd.clone())),
            Box::new(tools::codesearch::CodeSearchTool),
        ];

        if matches!(agent_type, AgentType::Build | AgentType::General) {
            tools.push(Box::new(tools::write::WriteTool));
            tools.push(Box::new(tools::multiedit::MultiEditTool));
            tools.push(Box::new(tools::edit::EditTool));
            tools.push(Box::new(tools::batch::BatchTool::new(self.cwd.clone())));
            tools.push(Box::new(tools::agent::AgentTool::<C>::new(
                client.clone(),
                config.clone(),
                self.cwd.clone(),
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
