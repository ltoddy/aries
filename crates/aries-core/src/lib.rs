pub mod agent_type;
pub mod compaction;
pub mod tools;

use anyhow::Context;
use aries_config::AriesConfig;
use rig::agent::{Agent, PromptHook, StreamingResult};
use rig::client::CompletionClient;
use rig::completion::{self, Message, Prompt};
use rig::providers::openai;
use rig::streaming::StreamingPrompt;

use crate::agent_type::AgentType;

pub const AGENT_LOOP_MAX_TURNS: usize = 200;

pub struct AgentWrapper<P = ()> {
    pub name: String,
    pub inner: Agent<openai::CompletionModel>,
    hook: P,
}

impl<P> AgentWrapper<P>
where
    P: PromptHook<openai::CompletionModel> + Clone + 'static,
{
    pub fn new(
        name: String,
        config: AriesConfig,
        agent_type: AgentType,
        hook: P,
    ) -> anyhow::Result<Self> {
        let client = openai::CompletionsClient::builder()
            .base_url(&config.base_url)
            .api_key(&config.api_key)
            .build()
            .with_context(|| "Failed to create llm client")?;

        let preamble = agent_type.system_prompt();
        let tools = agent_type.tools(config.clone(), hook.clone());

        let inner = client
            .agent(&config.model)
            .preamble(preamble)
            .tools(tools)
            .default_max_turns(AGENT_LOOP_MAX_TURNS)
            .build();

        Ok(Self { name, inner, hook })
    }

    #[inline]
    pub async fn stream_prompt(
        &mut self,
        prompt: &str,
        history: &[Message],
    ) -> StreamingResult<<openai::CompletionModel as completion::CompletionModel>::StreamingResponse>
    {
        self.inner
            .stream_prompt(prompt)
            .with_history(history.to_vec())
            .with_hook(self.hook.clone())
            .await
    }

    pub async fn prompt(&mut self, prompt: &str, history: &[Message]) -> anyhow::Result<String> {
        let res = self
            .inner
            .prompt(prompt)
            .with_history(&mut history.to_vec())
            .with_hook(self.hook.clone())
            .await?;
        Ok(res)
    }
}
