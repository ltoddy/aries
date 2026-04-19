use aries_config::AriesConfig;
use rig::agent::PromptHook;
use rig::client::CompletionClient;
use rig::completion;
use rig::tool::ToolDyn;

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

    pub fn with_tools(
        self,
        spawner: TaskSpawner,
        lsp_client: SharedLspClient,
    ) -> AgentWrapper<C::CompletionModel, P> {
        let model = self.config.model().to_owned();
        let agent_type = self.agent_type;
        let tools = build_tools::<C>(agent_type, self.config, &self.client, spawner, lsp_client);

        let inner = self
            .client
            .agent(model)
            .hook(self.hook)
            .name(agent_type.agent_name())
            .description(agent_type.description())
            .preamble(agent_type.preamble())
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
    lsp_client: SharedLspClient,
) -> Vec<Box<dyn ToolDyn>>
where
    C::CompletionModel: completion::CompletionModel + 'static,
{
    match agent_type {
        AgentType::Build | AgentType::General => vec![
            Box::new(tools::bash::ShellCommand),
            Box::new(tools::read::ReadFileTool),
            Box::new(tools::write::WriteFileTool),
            Box::new(tools::glob::GlobTool),
            Box::new(tools::grep::GrepTool),
            Box::new(tools::ls::LsTool),
            Box::new(tools::apply_patch::ApplyPatchTool),
            Box::new(tools::multiedit::MultiEditTool),
            Box::new(tools::edit::EditTool),
            Box::new(tools::batch::BatchTool::<C>::new(
                client.clone(),
                config.clone(),
                lsp_client.clone(),
            )),
            Box::new(tools::question::QuestionTool),
            Box::new(tools::task::TaskTool::<C>::new(client.clone(), config.clone())),
            Box::new(tools::lsp::LspTool::new(lsp_client.clone())),
            Box::new(tools::codesearch::CodeSearchTool),
            Box::new(tools::task_spawn::TaskSpawnTool::new(spawner.clone())),
            Box::new(tools::task_status::TaskStatusTool::new(spawner)),
        ],
        AgentType::Plan => vec![
            Box::new(tools::bash::ShellCommand),
            Box::new(tools::read::ReadFileTool),
            Box::new(tools::glob::GlobTool),
            Box::new(tools::grep::GrepTool),
            Box::new(tools::ls::LsTool),
            Box::new(tools::question::QuestionTool),
            Box::new(tools::lsp::LspTool::new(lsp_client.clone())),
            Box::new(tools::codesearch::CodeSearchTool),
        ],
        AgentType::Explore => vec![
            Box::new(tools::bash::ShellCommand),
            Box::new(tools::read::ReadFileTool),
            Box::new(tools::glob::GlobTool),
            Box::new(tools::grep::GrepTool),
            Box::new(tools::ls::LsTool),
            Box::new(tools::codesearch::CodeSearchTool),
        ],
        _ => vec![],
    }
}
