use aries_config::AriesConfig;
use aries_context::GlobalContext;
use rig::agent::PromptHook;
use rig::client::CompletionClient;
use rig::completion;
use rig::tool::ToolDyn;

use crate::agent_type::AgentType;
use crate::ext::skill::{SkillFilesLoader, SkillInfo};
use crate::language_server::SharedLspClient;
use crate::task_spawner::TaskSpawner;
use crate::{AGENT_LOOP_MAX_TURNS, AriesAgent, tools};

pub struct AgentBuilder<C, P>
where
    C: CompletionClient,
    C::CompletionModel: completion::CompletionModel,
    P: PromptHook<C::CompletionModel>,
{
    pub(crate) client: C,
    pub(crate) config: AriesConfig,
    pub(crate) agent_type: AgentType,
    pub(crate) hook: P,
    pub(crate) gctx: GlobalContext,
}

impl<C, P> AgentBuilder<C, P>
where
    C: CompletionClient + Clone + Send + Sync + 'static,
    C::CompletionModel: completion::CompletionModel + 'static,
    P: PromptHook<C::CompletionModel> + 'static,
{
    pub fn new(
        client: C,
        config: AriesConfig,
        agent_type: AgentType,
        hook: P,
        gctx: GlobalContext,
    ) -> Self {
        Self { client, config, agent_type, hook, gctx }
    }

    pub fn build(self) -> AriesAgent<C::CompletionModel, P> {
        let model = self.config.model().to_owned();
        let agent_type = self.agent_type;

        let preamble = agent_type.bare_preamble().to_owned();

        let inner = self
            .client
            .agent(model)
            .hook(self.hook)
            .name(agent_type.name())
            .description(agent_type.description())
            .preamble(&preamble)
            .default_max_turns(AGENT_LOOP_MAX_TURNS)
            .build();

        AriesAgent::new(inner, preamble)
    }

    pub async fn with_tools(
        self,
        spawner: TaskSpawner,
        lsp_client: Option<SharedLspClient>,
    ) -> AriesAgent<C::CompletionModel, P> {
        let model = self.config.model().to_owned();
        let agent_type = self.agent_type;

        let skillloader = SkillFilesLoader::new(&self.gctx);
        let available_skills = skillloader.load().await.unwrap_or_default();

        let preamble =
            crate::preamble::render(&self.gctx, agent_type, &model, &available_skills).await;

        let tools = self.build_tools(spawner, lsp_client, available_skills);

        let inner = self
            .client
            .agent(model)
            .hook(self.hook)
            .name(agent_type.name())
            .description(agent_type.description())
            .preamble(&preamble)
            .tools(tools)
            .default_max_turns(AGENT_LOOP_MAX_TURNS)
            .build();

        AriesAgent::new(inner, preamble)
    }

    fn build_tools(
        &self,
        spawner: TaskSpawner,
        lsp_client: Option<SharedLspClient>,
        available_skills: Vec<SkillInfo>,
    ) -> Vec<Box<dyn ToolDyn>> {
        let agent_type = self.agent_type;
        let config = self.config.clone();
        let client = self.client.clone();
        let gctx = self.gctx.clone();

        match agent_type {
            AgentType::Build | AgentType::General => {
                let mut tools: Vec<Box<dyn ToolDyn>> = vec![
                    Box::new(tools::bash::ShellCommandTool),
                    Box::new(tools::read::ReadFileTool),
                    Box::new(tools::write::WriteFileTool),
                    Box::new(tools::glob::GlobTool::new(gctx.clone())),
                    Box::new(tools::grep::GrepTool::new(gctx.clone())),
                    Box::new(tools::ls::LsTool::new(gctx.clone())),
                    Box::new(tools::apply_patch::ApplyPatchTool),
                    Box::new(tools::multiedit::MultiEditTool),
                    Box::new(tools::edit::EditTool),
                    Box::new(tools::batch::BatchTool::<C>::new(
                        client.clone(),
                        config.clone(),
                        gctx.clone(),
                    )),
                    Box::new(tools::question::QuestionTool),
                    Box::new(tools::task::TaskTool::<C>::new(
                        client.clone(),
                        config.clone(),
                        gctx.clone(),
                    )),
                    Box::new(tools::codesearch::CodeSearchTool),
                    Box::new(tools::task_spawn::TaskSpawnTool::new(spawner.clone())),
                    Box::new(tools::task_status::TaskStatusTool::new(spawner)),
                ];
                if !available_skills.is_empty() {
                    tools.push(Box::new(tools::skill::SkillTool::new(available_skills)))
                }
                if let Some(lsp_client) = lsp_client {
                    tools.push(Box::new(tools::lsp::LspTool::new(lsp_client, gctx.clone())));
                }
                tools
            },
            AgentType::Plan => {
                let mut tools: Vec<Box<dyn ToolDyn>> = vec![
                    Box::new(tools::bash::ShellCommandTool),
                    Box::new(tools::read::ReadFileTool),
                    Box::new(tools::glob::GlobTool::new(gctx.clone())),
                    Box::new(tools::grep::GrepTool::new(gctx.clone())),
                    Box::new(tools::ls::LsTool::new(gctx.clone())),
                    Box::new(tools::question::QuestionTool),
                    Box::new(tools::codesearch::CodeSearchTool),
                ];
                if !available_skills.is_empty() {
                    tools.push(Box::new(tools::skill::SkillTool::new(available_skills)))
                }
                if let Some(lsp_client) = lsp_client {
                    tools.push(Box::new(tools::lsp::LspTool::new(lsp_client, gctx.clone())));
                }
                tools
            },
            AgentType::Explore => vec![
                Box::new(tools::bash::ShellCommandTool),
                Box::new(tools::read::ReadFileTool),
                Box::new(tools::glob::GlobTool::new(gctx.clone())),
                Box::new(tools::grep::GrepTool::new(gctx.clone())),
                Box::new(tools::ls::LsTool::new(gctx.clone())),
                Box::new(tools::codesearch::CodeSearchTool),
            ],
            _ => vec![],
        }
    }
}
