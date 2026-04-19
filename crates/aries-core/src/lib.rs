pub mod compaction;
pub mod language_server;
pub mod rpc;
pub mod task_spawner;
pub mod tools;

use aries_config::AriesConfig;
use rig::agent::{Agent, PromptHook, StreamingResult};
use rig::client::CompletionClient;
use rig::completion::{self, Message, Prompt};
use rig::streaming::StreamingPrompt;
use rig::tool::ToolDyn;
use task_spawner::TaskSpawner;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentType {
    Build,
    Plan,
    General,
    Explore,
    Compaction,
    Title,
    Summary,
}

pub const AGENT_LOOP_MAX_TURNS: usize = 200;

pub struct AgentWrapper<M, P = ()>
where
    M: completion::CompletionModel,
    P: PromptHook<M>,
{
    inner: Agent<M, P>,
}

impl<M, P> AgentWrapper<M, P>
where
    M: completion::CompletionModel + 'static,
    P: PromptHook<M> + 'static,
{
    pub fn new<C: CompletionClient<CompletionModel = M> + Clone + Send + Sync + 'static>(
        client: C,
        config: AriesConfig,
        agent_type: AgentType,
        hook: P,
        spawner: TaskSpawner,
    ) -> Self {
        let model = config.model().to_owned();
        let preamble = Self::preamble(agent_type);
        let tools = Self::tools::<C>(agent_type, config, &client, spawner);

        let inner = client
            .agent(model)
            .hook(hook)
            .name(Self::name(agent_type))
            .description(Self::description(agent_type))
            .preamble(preamble)
            .tools(tools)
            .default_max_turns(AGENT_LOOP_MAX_TURNS)
            .build();

        Self { inner }
    }

    pub async fn stream_prompt(
        &mut self,
        prompt: &str,
        history: &[Message],
    ) -> StreamingResult<<M>::StreamingResponse> {
        self.inner.stream_prompt(prompt).with_history(history.to_vec()).await
    }

    pub async fn prompt(&mut self, prompt: &str, history: &[Message]) -> anyhow::Result<String> {
        let res = self.inner.prompt(prompt).with_history(&mut history.to_vec()).await?;
        Ok(res)
    }

    const fn preamble(agent_type: AgentType) -> &'static str {
        match agent_type {
            AgentType::Build => include_str!("prompts/build.txt"),
            AgentType::Plan => include_str!("prompts/plan.txt"),
            AgentType::General => include_str!("prompts/generate.txt"),
            AgentType::Explore => include_str!("prompts/explore.txt"),
            AgentType::Compaction => include_str!("prompts/compaction.txt"),
            AgentType::Title => include_str!("prompts/title.txt"),
            AgentType::Summary => include_str!("prompts/summary.txt"),
        }
    }

    const fn name(agent_type: AgentType) -> &'static str {
        match agent_type {
            AgentType::Build => "Builder",
            AgentType::Plan => "Planner",
            AgentType::General => "Assistant",
            AgentType::Explore => "Explorer",
            AgentType::Compaction => "Archivist",
            AgentType::Title => "Namer",
            AgentType::Summary => "Summarizer",
        }
    }

    const fn description(agent_type: AgentType) -> &'static str {
        match agent_type {
            AgentType::Build => "默认主智能体。直接使用工具执行任务，并在需要时委托子智能体。",
            AgentType::Plan => "计划模式。不允许使用所有编辑工具。",
            AgentType::General => {
                "用于研究复杂问题和执行多步任务的通用智能体。使用此智能体并行执行多个工作单元。"
            },
            AgentType::Explore => {
                "专门用于探索代码库的快速智能体。当您需要通过模式快速查找文件、搜索关键字或回答有关代码库的问题时使用。"
            },
            AgentType::Compaction => "用于压缩和总结对话上下文的智能体。",
            AgentType::Title => "用于生成对话标题的智能体。",
            AgentType::Summary => "用于生成对话摘要（类似于 PR 描述）的智能体。",
        }
    }

    fn tools<C: CompletionClient<CompletionModel = M> + Clone + Send + Sync + 'static>(
        agent_type: AgentType,
        config: AriesConfig,
        client: &C,
        spawner: TaskSpawner,
    ) -> Vec<Box<dyn ToolDyn>> {
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
                Box::new(tools::batch::BatchTool::<C>::new(client.clone(), config.clone())),
                Box::new(tools::question::QuestionTool),
                Box::new(tools::task::TaskTool::<C>::new(client.clone(), config.clone())),
                Box::new(tools::lsp::LspTool),
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
                Box::new(tools::lsp::LspTool),
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
}
