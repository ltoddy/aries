pub mod compaction;
pub mod summary;
pub mod title;

use aries_config::AriesConfig;
use aries_context::GlobalContext;
use futures::StreamExt;
use rig::agent::{Agent, FinalResponse, MultiTurnStreamItem, PromptHook, StreamingResult};
use rig::client::CompletionClient;
use rig::completion::{self, Message};
use rig::streaming::StreamingPrompt;
use rig::tool::ToolDyn;
use rig::wasm_compat::WasmCompatSend;

pub use self::compaction::CompactionAgent;
pub use self::summary::SummaryAgent;
pub use self::title::TitleAgent;
use crate::ext::skill::{SkillFilesLoader, SkillInfo};
use crate::language_server::SharedLspClient;
use crate::tools;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentType {
    Build,
    Plan,
    General,
    Explore,
}

impl AgentType {
    pub const fn bare_preamble(self) -> &'static str {
        match self {
            Self::Build => include_str!("prompts/build.txt"),
            Self::Plan => include_str!("prompts/plan.txt"),
            Self::General => include_str!("prompts/generate.txt"),
            Self::Explore => include_str!("prompts/explore.txt"),
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Build => "Builder",
            Self::Plan => "Planner",
            Self::General => "Assistant",
            Self::Explore => "Explorer",
        }
    }

    pub const fn description(self) -> &'static str {
        match self {
            Self::Build => "默认主智能体。直接使用工具执行任务，并在需要时委托子智能体。",
            Self::Plan => "计划模式。不允许使用所有编辑工具。",
            Self::General => {
                "用于研究复杂问题和执行多步任务的通用智能体。使用此智能体并行执行多个工作单元。"
            },
            Self::Explore => {
                "专门用于探索代码库的快速智能体。当您需要通过模式快速查找文件、搜索关键字或回答有关代码库的问题时使用。"
            },
        }
    }
}

pub const AGENT_LOOP_MAX_TURNS: usize = 200;

#[derive(Clone)]
pub struct AriesAgent<M>
where
    M: completion::CompletionModel,
{
    inner: Agent<M>,
    preamble: String,
}

impl<M> AriesAgent<M>
where
    M: completion::CompletionModel,
{
    pub fn new(inner: Agent<M>, preamble: String) -> Self {
        Self { inner, preamble }
    }
}

impl<M> AriesAgent<M>
where
    M: completion::CompletionModel + 'static,
{
    pub async fn stream_prompt<P>(
        &mut self,
        prompt: impl Into<Message> + WasmCompatSend,
        history: &[Message],
        hook: P,
    ) -> StreamingResult<<M>::StreamingResponse>
    where
        P: PromptHook<M> + 'static,
    {
        self.inner.stream_prompt(prompt).with_history(history).with_hook(hook).await
    }

    pub async fn complete(
        &mut self,
        prompt: impl Into<Message> + WasmCompatSend,
        history: &[Message],
    ) -> anyhow::Result<String> {
        let stream = self.inner.stream_prompt(prompt).with_history(history).await;
        futures::pin_mut!(stream);

        let mut final_res = FinalResponse::empty();
        while let Some(item) = stream.next().await {
            let item = item?;

            if let MultiTurnStreamItem::FinalResponse(res) = item {
                final_res = res;
            }
        }

        Ok(final_res.response().to_owned())
    }

    #[inline]
    pub fn system_prompt(&self) -> &str {
        &self.preamble
    }
}

pub struct AgentBuilder<C>
where
    C: CompletionClient,
    C::CompletionModel: completion::CompletionModel,
{
    pub(crate) client: C,
    pub(crate) config: AriesConfig,
    pub(crate) agent_type: AgentType,
    pub(crate) gctx: GlobalContext,
}

impl<C> AgentBuilder<C>
where
    C: CompletionClient + Clone + Send + Sync + 'static,
    C::CompletionModel: completion::CompletionModel + 'static,
{
    pub fn new(client: C, config: AriesConfig, agent_type: AgentType, gctx: GlobalContext) -> Self {
        Self { client, config, agent_type, gctx }
    }

    pub fn build(self) -> AriesAgent<C::CompletionModel> {
        let model = self.config.model().to_owned();
        let agent_type = self.agent_type;

        let preamble = agent_type.bare_preamble().to_owned();

        let inner = self
            .client
            .agent(model)
            .name(agent_type.name())
            .description(agent_type.description())
            .preamble(&preamble)
            .default_max_turns(AGENT_LOOP_MAX_TURNS)
            .build();

        AriesAgent::new(inner, preamble)
    }

    pub async fn with_tools(
        self,
        lsp_client: Option<SharedLspClient>,
    ) -> AriesAgent<C::CompletionModel> {
        let model = self.config.model().to_owned();
        let agent_type = self.agent_type;

        let skillloader = SkillFilesLoader::new(&self.gctx);
        let available_skills = skillloader.load().await.unwrap_or_default();

        let preamble =
            crate::preamble::render(&self.gctx, agent_type, &model, &available_skills).await;

        let tools = self.build_tools(lsp_client, available_skills);

        let inner = self
            .client
            .agent(model)
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
        lsp_client: Option<SharedLspClient>,
        available_skills: Vec<SkillInfo>,
    ) -> Vec<Box<dyn ToolDyn>> {
        let agent_type = self.agent_type;
        let config = self.config.clone();
        let client = self.client.clone();
        let gctx = self.gctx.clone();

        let mut tools: Vec<Box<dyn ToolDyn>> = vec![
            Box::new(tools::bash::ShellCommandTool),
            Box::new(tools::read::ReadFileTool),
            Box::new(tools::glob::GlobTool::new(gctx.clone())),
            Box::new(tools::grep::GrepTool::new(gctx.clone())),
            Box::new(tools::ls::LsTool::new(gctx.clone())),
            Box::new(tools::codesearch::CodeSearchTool),
        ];

        if matches!(agent_type, AgentType::Build | AgentType::General) {
            tools.push(Box::new(tools::write::WriteFileTool));
            tools.push(Box::new(tools::apply_patch::ApplyPatchTool));
            tools.push(Box::new(tools::multiedit::MultiEditTool));
            tools.push(Box::new(tools::edit::EditTool));
            tools.push(Box::new(tools::batch::BatchTool::new(gctx.clone())));
            tools.push(Box::new(tools::task::TaskTool::<C>::new(
                client.clone(),
                config.clone(),
                gctx.clone(),
            )));
        }

        if matches!(agent_type, AgentType::Build | AgentType::General | AgentType::Plan) {
            tools.push(Box::new(tools::question::QuestionTool));
        }

        if matches!(agent_type, AgentType::Build | AgentType::General) {
            if !available_skills.is_empty() {
                tools.push(Box::new(tools::skill::SkillTool::new(available_skills)));
            }
            if let Some(lsp_client) = lsp_client {
                tools.push(Box::new(tools::lsp::LspTool::new(lsp_client, gctx.clone())));
            }
        }

        tools
    }
}
