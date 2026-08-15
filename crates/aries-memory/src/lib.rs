mod agent;
mod retriever;
mod store;

use reqwest_middleware::ClientWithMiddleware;
use rig_core::providers::{anthropic, azure, deepseek, openai};

pub use self::agent::MemoryAgent;
pub use self::retriever::MemoryRetriever;
pub use self::store::{Memory, MemoryFrontmatter, MemoryStore, MemoryType};

pub enum MemoryAgentProvider {
    Anthropic(MemoryAgent<anthropic::completion::CompletionModel<ClientWithMiddleware>>),
    Azure(MemoryAgent<azure::CompletionModel<ClientWithMiddleware>>),
    Deepseek(MemoryAgent<deepseek::CompletionModel<ClientWithMiddleware>>),
    OpenAI(MemoryAgent<openai::CompletionModel<ClientWithMiddleware>>),
}

impl MemoryAgentProvider {
    #[inline]
    pub async fn run(
        &self,
        manifest: Option<String>,
        user: impl Into<String>,
        assistant: impl Into<String>,
    ) {
        match self {
            MemoryAgentProvider::Anthropic(a) => a.run(manifest, user, assistant).await,
            MemoryAgentProvider::Azure(a) => a.run(manifest, user, assistant).await,
            MemoryAgentProvider::Deepseek(a) => a.run(manifest, user, assistant).await,
            MemoryAgentProvider::OpenAI(a) => a.run(manifest, user, assistant).await,
        }
    }
}

pub enum MemoryRetrieverProvider {
    Anthropic(MemoryRetriever<anthropic::completion::CompletionModel<ClientWithMiddleware>>),
    Azure(MemoryRetriever<azure::CompletionModel<ClientWithMiddleware>>),
    Deepseek(MemoryRetriever<deepseek::CompletionModel<ClientWithMiddleware>>),
    OpenAI(MemoryRetriever<openai::CompletionModel<ClientWithMiddleware>>),
}

impl MemoryRetrieverProvider {
    pub async fn retrieve(&self, query: impl Into<String>, memories: &[Memory]) -> Vec<String> {
        match self {
            MemoryRetrieverProvider::Anthropic(a) => a.retrieve(query, memories).await,
            MemoryRetrieverProvider::Azure(a) => a.retrieve(query, memories).await,
            MemoryRetrieverProvider::Deepseek(a) => a.retrieve(query, memories).await,
            MemoryRetrieverProvider::OpenAI(a) => a.retrieve(query, memories).await,
        }
    }
}
