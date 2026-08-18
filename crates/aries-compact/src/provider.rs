use rig::completion::Message;

use crate::{CompactAgent, CompactOutcome};

#[derive(Clone)]
pub enum CompactAgentProvider {
    Anthropic(CompactAgent),
    Azure(CompactAgent),
    Deepseek(CompactAgent),
    OpenAI(CompactAgent),
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
