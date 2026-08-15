use reqwest_middleware::ClientWithMiddleware;
use rig_core::completion::Message;
use rig_core::providers::{anthropic, azure, deepseek, openai};

use crate::{CompactAgent, CompactOutcome};

#[derive(Clone)]
pub enum CompactAgentProvider {
    Anthropic(CompactAgent<anthropic::completion::CompletionModel<ClientWithMiddleware>>),
    Azure(CompactAgent<azure::CompletionModel<ClientWithMiddleware>>),
    Deepseek(CompactAgent<deepseek::CompletionModel<ClientWithMiddleware>>),
    OpenAI(CompactAgent<openai::CompletionModel<ClientWithMiddleware>>),
}

impl CompactAgentProvider {
    pub async fn compact(&mut self, messages: &[Message]) -> CompactOutcome {
        match self {
            CompactAgentProvider::Anthropic(a) => a.compact(messages).await,
            CompactAgentProvider::Azure(a) => a.compact(messages).await,
            CompactAgentProvider::Deepseek(a) => a.compact(messages).await,
            CompactAgentProvider::OpenAI(a) => a.compact(messages).await,
        }
    }
}
