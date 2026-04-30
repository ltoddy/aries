use aries_config::AriesConfig;
use aries_context::GlobalContext;
use rig::agent::PromptHook;
use rig::client::CompletionClient;
use rig::completion;
use rig::tool::ToolDyn;

use crate::ext::skill;
use crate::ext::skill::{SkillFilesLoader, SkillInfo};
use crate::language_server::SharedLspClient;
use crate::task_spawner::TaskSpawner;
use crate::{AGENT_LOOP_MAX_TURNS, AgentType, AgentWrapper, tools};

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
    pub fn build(self) -> AgentWrapper<C::CompletionModel, P> {
        let model = self.config.model().to_owned();
        let agent_type = self.agent_type;

        let inner = self
            .client
            .agent(model)
            .hook(self.hook)
            .name(agent_type.agent_name())
            .description(agent_type.description())
            .preamble(agent_type.preamble())
            .default_max_turns(AGENT_LOOP_MAX_TURNS)
            .build();

        AgentWrapper { inner }
    }

    pub async fn with_tools(
        self,
        spawner: TaskSpawner,
        lsp_client: Option<SharedLspClient>,
    ) -> AgentWrapper<C::CompletionModel, P> {
        let model = self.config.model().to_owned();
        let agent_type = self.agent_type;

        let skillloader = SkillFilesLoader::new(&self.gctx);
        let available_skills = skillloader.load().await.unwrap_or_default();

        let preamble = crate::preamble::render(&self.gctx, agent_type, &available_skills).await;

        let tools = build_tools::<C>(
            agent_type,
            self.config,
            &self.client,
            spawner,
            lsp_client,
            available_skills,
            self.gctx.clone(),
        );

        let inner = self
            .client
            .agent(model)
            .hook(self.hook)
            .name(agent_type.agent_name())
            .description(agent_type.description())
            .preamble(&preamble)
            .tools(tools)
            .default_max_turns(AGENT_LOOP_MAX_TURNS)
            .build();

        AgentWrapper { inner }
    }
}

fn build_tools<C: CompletionClient + Clone + Send + Sync + 'static>(
    agent_type: AgentType,
    config: AriesConfig,
    client: &C,
    spawner: TaskSpawner,
    lsp_client: Option<SharedLspClient>,
    available_skills: Vec<SkillInfo>,
    gctx: GlobalContext,
) -> Vec<Box<dyn ToolDyn>>
where
    C::CompletionModel: completion::CompletionModel + 'static,
{
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
