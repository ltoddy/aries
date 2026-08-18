use rig::agent::{AgentHook, PromptResponse};
use rig::completion::Message;

use crate::{AriesAgent, AriesResult};

#[derive(Clone)]
pub enum AriesAgentProvider {
    Anthropic(AriesAgent),
    Azure(AriesAgent),
    Deepseek(AriesAgent),
    OpenAI(AriesAgent),
}

impl AriesAgentProvider {
    #[inline]
    pub async fn prompt<I, T, P>(
        &self,
        prompt: impl Into<Message> + Send,
        history: I,
        hook: P,
    ) -> AriesResult<PromptResponse>
    where
        I: IntoIterator<Item = T>,
        T: Into<Message>,
        P: AgentHook + 'static,
    {
        match self {
            AriesAgentProvider::Anthropic(a) => a.prompt(prompt, history, hook).await,
            AriesAgentProvider::Azure(a) => a.prompt(prompt, history, hook).await,
            AriesAgentProvider::Deepseek(a) => a.prompt(prompt, history, hook).await,
            AriesAgentProvider::OpenAI(a) => a.prompt(prompt, history, hook).await,
        }
    }

    #[inline]
    pub fn preamble(&self) -> &str {
        match self {
            AriesAgentProvider::Anthropic(a) => a.preamble(),
            AriesAgentProvider::Azure(a) => a.preamble(),
            AriesAgentProvider::Deepseek(a) => a.preamble(),
            AriesAgentProvider::OpenAI(a) => a.preamble(),
        }
    }
}
