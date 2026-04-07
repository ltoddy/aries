pub mod agent_type;
pub mod compaction;
pub mod tools;

use std::marker::PhantomData;

use anyhow::{Context, bail};
use aries_config::AriesConfig;
use rig::agent::{Agent, PromptHook, StreamingResult};
use rig::client::CompletionClient;
use rig::completion::{self, Message, Prompt};
use rig::providers::{azure, openai};
use rig::streaming::StreamingPrompt;
use rig::tool::ToolDyn;

use crate::agent_type::AgentType;
use crate::tools::{
    ApplyPatchTool, BatchTool, CodeSearchTool, EditTool, GlobTool, GrepTool, LsTool, LspTool, MultiEditTool,
    QuestionTool, ReadFileTool, ShellCommand, TaskTool, WebFetchTool, WebSearchTool, WriteFileTool,
};

pub const AGENT_LOOP_MAX_TURNS: usize = 200;

pub struct AgentWrapper<M, P = ()>
where
    M: completion::CompletionModel,
    P: PromptHook<M>,
{
    name: String,
    pub inner: Agent<M>,
    hook: P,
    _phantom: PhantomData<M>,
}

impl<M, P> AgentWrapper<M, P>
where
    M: completion::CompletionModel + 'static,
    P: PromptHook<M> + 'static,
{
    pub fn name(&self) -> &str {
        &self.name
    }
}

impl<P> AgentWrapper<openai::CompletionModel, P>
where
    P: PromptHook<openai::CompletionModel> + 'static,
{
    pub fn new(name: String, config: AriesConfig, agent_type: AgentType, hook: P) -> anyhow::Result<Self> {
        let AriesConfig::OpenAICompatible(config) = config else {
            bail!("OpenAI compatible agent requires an OpenAI compatible config");
        };
        let tool_config = AriesConfig::OpenAICompatible(config.clone());

        let client = openai::CompletionsClient::builder()
            .base_url(&config.base_url)
            .api_key(&config.api_key)
            .build()
            .with_context(|| "Failed to create llm client")?;

        let preamble = agent_type.preamble();
        let tools: Vec<Box<dyn ToolDyn>> = match agent_type {
            AgentType::Orchestrate => {
                vec![
                    Box::new(QuestionTool),
                    Box::new(TaskTool::<openai::CompletionModel, P>::new(tool_config.clone(), hook.clone())),
                ]
            },
            AgentType::Build | AgentType::General => vec![
                Box::new(ShellCommand),
                Box::new(ReadFileTool),
                Box::new(WriteFileTool),
                Box::new(GlobTool),
                Box::new(GrepTool),
                Box::new(LsTool),
                Box::new(ApplyPatchTool),
                Box::new(MultiEditTool),
                Box::new(EditTool),
                Box::new(BatchTool::<openai::CompletionModel, P>::new(tool_config.clone(), hook.clone())),
                Box::new(QuestionTool),
                Box::new(TaskTool::<openai::CompletionModel, P>::new(tool_config.clone(), hook.clone())),
                Box::new(WebFetchTool),
                Box::new(WebSearchTool),
                Box::new(LspTool),
                Box::new(CodeSearchTool),
            ],
            AgentType::Plan => vec![
                Box::new(ShellCommand),
                Box::new(ReadFileTool),
                Box::new(GlobTool),
                Box::new(GrepTool),
                Box::new(LsTool),
                Box::new(QuestionTool),
                Box::new(WebFetchTool),
                Box::new(WebSearchTool),
                Box::new(LspTool),
                Box::new(CodeSearchTool),
            ],
            AgentType::Explore => vec![
                Box::new(ReadFileTool),
                Box::new(GlobTool),
                Box::new(GrepTool),
                Box::new(LsTool),
                Box::new(WebFetchTool),
                Box::new(WebSearchTool),
                Box::new(CodeSearchTool),
            ],
            AgentType::Compaction | AgentType::Title | AgentType::Summary => vec![],
        };

        let inner = client
            .agent(&config.model)
            .name(agent_type.name())
            .description(agent_type.description())
            .preamble(preamble)
            .tools(tools)
            .default_max_turns(AGENT_LOOP_MAX_TURNS)
            .build();

        Ok(Self { name, inner, hook, _phantom: Default::default() })
    }

    #[inline]
    pub async fn stream_prompt(
        &mut self,
        prompt: &str,
        history: &[Message],
    ) -> StreamingResult<<openai::CompletionModel as completion::CompletionModel>::StreamingResponse> {
        self.inner.stream_prompt(prompt).with_history(history.to_vec()).with_hook(self.hook.clone()).await
    }

    pub async fn prompt(&mut self, prompt: &str, history: &[Message]) -> anyhow::Result<String> {
        let res = self.inner.prompt(prompt).with_history(&mut history.to_vec()).with_hook(self.hook.clone()).await?;
        Ok(res)
    }
}

impl<P> AgentWrapper<azure::CompletionModel, P>
where
    P: PromptHook<azure::CompletionModel> + 'static,
{
    pub fn new(name: String, config: AriesConfig, agent_type: AgentType, hook: P) -> anyhow::Result<Self> {
        let AriesConfig::Azure(config) = config else {
            bail!("Azure agent requires an Azure config");
        };
        let tool_config = AriesConfig::Azure(config.clone());

        let client = azure::Client::builder()
            .api_key(&config.api_key)
            .azure_endpoint(config.azure_endpoint)
            .api_version(&config.api_version)
            .build()
            .with_context(|| "Failed to create llm client")?;

        let preamble = agent_type.preamble();

        let tools: Vec<Box<dyn ToolDyn>> = match agent_type {
            AgentType::Orchestrate => {
                vec![
                    Box::new(QuestionTool),
                    Box::new(TaskTool::<azure::CompletionModel, P>::new(tool_config.clone(), hook.clone())),
                ]
            },
            AgentType::Build | AgentType::General => vec![
                Box::new(ShellCommand),
                Box::new(ReadFileTool),
                Box::new(WriteFileTool),
                Box::new(GlobTool),
                Box::new(GrepTool),
                Box::new(LsTool),
                Box::new(ApplyPatchTool),
                Box::new(MultiEditTool),
                Box::new(EditTool),
                Box::new(BatchTool::<azure::CompletionModel, P>::new(tool_config.clone(), hook.clone())),
                Box::new(QuestionTool),
                Box::new(TaskTool::<azure::CompletionModel, P>::new(tool_config.clone(), hook.clone())),
                Box::new(WebFetchTool),
                Box::new(WebSearchTool),
                Box::new(LspTool),
                Box::new(CodeSearchTool),
            ],
            AgentType::Plan => vec![
                Box::new(ShellCommand),
                Box::new(ReadFileTool),
                Box::new(GlobTool),
                Box::new(GrepTool),
                Box::new(LsTool),
                Box::new(QuestionTool),
                Box::new(WebFetchTool),
                Box::new(WebSearchTool),
                Box::new(LspTool),
                Box::new(CodeSearchTool),
            ],
            AgentType::Explore => vec![
                Box::new(ReadFileTool),
                Box::new(GlobTool),
                Box::new(GrepTool),
                Box::new(LsTool),
                Box::new(WebFetchTool),
                Box::new(WebSearchTool),
                Box::new(CodeSearchTool),
            ],
            AgentType::Compaction | AgentType::Title | AgentType::Summary => vec![],
        };

        let inner = client
            .agent(&config.mode)
            .name(agent_type.name())
            .description(agent_type.description())
            .preamble(preamble)
            .tools(tools)
            .default_max_turns(AGENT_LOOP_MAX_TURNS)
            .build();

        Ok(Self { name, inner, hook, _phantom: Default::default() })
    }

    #[inline]
    pub async fn stream_prompt(
        &mut self,
        prompt: &str,
        history: &[Message],
    ) -> StreamingResult<<azure::CompletionModel as completion::CompletionModel>::StreamingResponse> {
        self.inner.stream_prompt(prompt).with_history(history.to_vec()).with_hook(self.hook.clone()).await
    }

    pub async fn prompt(&mut self, prompt: &str, history: &[Message]) -> anyhow::Result<String> {
        let res = self.inner.prompt(prompt).with_history(&mut history.to_vec()).with_hook(self.hook.clone()).await?;
        Ok(res)
    }
}
