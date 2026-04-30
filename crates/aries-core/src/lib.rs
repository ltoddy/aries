pub mod build;
pub mod compaction;
pub mod ext;
pub mod fs;
pub mod jsonl;
pub mod language_server;
pub mod preamble;
pub mod rpc;
pub mod task_spawner;
pub mod tools;

use aries_context::GlobalContext;
use rig::agent::{Agent, PromptHook, StreamingResult};
use rig::client::CompletionClient;
use rig::completion::{self, Message, Prompt};
use rig::streaming::StreamingPrompt;

use crate::build::AgentBuilder;

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

impl AgentType {
    const fn preamble(self) -> &'static str {
        match self {
            Self::Build => include_str!("prompts/build.txt"),
            Self::Plan => include_str!("prompts/plan.txt"),
            Self::General => include_str!("prompts/generate.txt"),
            Self::Explore => include_str!("prompts/explore.txt"),
            Self::Compaction => include_str!("prompts/compaction.txt"),
            Self::Title => include_str!("prompts/title.txt"),
            Self::Summary => include_str!("prompts/summary.txt"),
        }
    }

    const fn agent_name(self) -> &'static str {
        match self {
            Self::Build => "Builder",
            Self::Plan => "Planner",
            Self::General => "Assistant",
            Self::Explore => "Explorer",
            Self::Compaction => "Archivist",
            Self::Title => "Namer",
            Self::Summary => "Summarizer",
        }
    }

    const fn description(self) -> &'static str {
        match self {
            Self::Build => "默认主智能体。直接使用工具执行任务，并在需要时委托子智能体。",
            Self::Plan => "计划模式。不允许使用所有编辑工具。",
            Self::General => {
                "用于研究复杂问题和执行多步任务的通用智能体。使用此智能体并行执行多个工作单元。"
            },
            Self::Explore => {
                "专门用于探索代码库的快速智能体。当您需要通过模式快速查找文件、搜索关键字或回答有关代码库的问题时使用。"
            },
            Self::Compaction => "用于压缩和总结对话上下文的智能体。",
            Self::Title => "用于生成对话标题的智能体。",
            Self::Summary => "用于生成对话摘要（类似于 PR 描述）的智能体。",
        }
    }
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
    #[allow(clippy::new_ret_no_self)]
    pub fn new<C: CompletionClient<CompletionModel = M> + Clone + Send + Sync + 'static>(
        client: C,
        config: aries_config::AriesConfig,
        agent_type: AgentType,
        hook: P,
        gctx: GlobalContext,
    ) -> AgentBuilder<C, P> {
        AgentBuilder { client, config, agent_type, hook, gctx }
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
}
