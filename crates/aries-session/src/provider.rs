use std::path::Path;

use aries_agent::{AgentBuilder, AriesAgent, AriesResult};
use aries_event::AgentEvent;
use aries_extension::AgentExtensions;
use aries_init::{GlobalContext, ModelConfig};
use aries_lspclient::SharedLspClient;
use aries_memory::{
    ExtractedMemory, ManifestEntry, MemoryExtractor, MemoryFrontmatter, MemoryStore,
};
use aries_mode::Mode;
use reqwest_middleware::ClientWithMiddleware;
use reqwest_retry::RetryTransientMiddleware;
use reqwest_retry::policies::ExponentialBackoff;
use rig_agent::agent::{AgentHook, PromptResponse};
use rig_agent::tool::server::ToolServerHandle;
use rig_core::completion::Message;
use rig_core::http_client;
use rig_core::providers::{anthropic, azure, deepseek, openai};
use tokio::sync::mpsc::UnboundedReceiver;

use crate::middleware::RetryStrategy;

#[derive(Clone)]
pub enum AriesClientProvider {
    Anthropic(anthropic::Client<ClientWithMiddleware>),
    Azure(azure::Client<ClientWithMiddleware>),
    Deepseek(deepseek::Client<ClientWithMiddleware>),
    OpenAI(openai::CompletionsClient<ClientWithMiddleware>),
}

impl AriesClientProvider {
    pub fn new(config: &ModelConfig) -> http_client::Result<Self> {
        let http_client = reqwest::Client::builder()
            .build()
            .expect("Failed to build http client for llm provider");

        let retry = RetryTransientMiddleware::new_with_policy_and_strategy(
            ExponentialBackoff::builder().base(1).build_with_max_retries(5),
            RetryStrategy::new(),
        );
        let httpclient = reqwest_middleware::ClientBuilder::new(http_client).with(retry).build();

        match config {
            ModelConfig::Anthropic(c) => {
                let client = anthropic::Client::builder()
                    .api_key(&c.api_key)
                    .base_url(&c.base_url)
                    .http_client(httpclient)
                    .build()?;
                Ok(AriesClientProvider::Anthropic(client))
            },
            ModelConfig::Azure(c) => {
                let client = azure::Client::builder()
                    .api_key(&c.api_key)
                    .azure_endpoint(c.azure_endpoint.clone())
                    .api_version(&c.api_version)
                    .http_client(httpclient)
                    .build()?;
                Ok(AriesClientProvider::Azure(client))
            },
            ModelConfig::Deepseek(c) => {
                let client = deepseek::Client::builder()
                    .api_key(&c.api_key)
                    .http_client(httpclient)
                    .build()?;
                Ok(AriesClientProvider::Deepseek(client))
            },
            ModelConfig::OpenAI(c) => {
                let client = openai::CompletionsClient::builder()
                    .base_url(&c.base_url)
                    .api_key(&c.api_key)
                    .http_client(httpclient)
                    .build()?;
                Ok(AriesClientProvider::OpenAI(client))
            },
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn agent(
        &self,
        mode: Mode,
        config: ModelConfig,
        cwd: impl AsRef<Path>,
        gctx: GlobalContext,
        lsp_client: Option<SharedLspClient>,
        extensions: AgentExtensions,
        tool_server_handle: ToolServerHandle,
    ) -> anyhow::Result<(AriesAgentProvider, UnboundedReceiver<AgentEvent>)> {
        let model = config.model();
        let cwd = cwd.as_ref().to_path_buf();

        match self {
            AriesClientProvider::Anthropic(c) => {
                let (agent, receiver) = AgentBuilder::new(c.clone(), &model, mode, cwd, gctx)
                    .with_lsp_client(lsp_client)
                    .with_extensions(extensions)
                    .build(tool_server_handle)
                    .await;
                Ok((AriesAgentProvider::Anthropic(agent), receiver))
            },
            AriesClientProvider::Azure(c) => {
                let (agent, receiver) = AgentBuilder::new(c.clone(), &model, mode, cwd, gctx)
                    .with_lsp_client(lsp_client)
                    .with_extensions(extensions)
                    .build(tool_server_handle)
                    .await;
                Ok((AriesAgentProvider::Azure(agent), receiver))
            },
            AriesClientProvider::Deepseek(c) => {
                let (agent, receiver) = AgentBuilder::new(c.clone(), &model, mode, cwd, gctx)
                    .with_lsp_client(lsp_client)
                    .with_extensions(extensions)
                    .build(tool_server_handle)
                    .await;
                Ok((AriesAgentProvider::Deepseek(agent), receiver))
            },
            AriesClientProvider::OpenAI(c) => {
                let (agent, receiver) = AgentBuilder::new(c.clone(), &model, mode, cwd, gctx)
                    .with_lsp_client(lsp_client)
                    .with_extensions(extensions)
                    .build(tool_server_handle)
                    .await;
                Ok((AriesAgentProvider::OpenAI(agent), receiver))
            },
        }
    }

    pub async fn extract_memories(
        &self,
        model: impl Into<String>,
        user: impl Into<String>,
        assistant: impl Into<String>,
        store: &MemoryStore,
    ) {
        let model = model.into();

        let memories = match self {
            AriesClientProvider::Anthropic(c) => {
                let extractor = MemoryExtractor::new(c.clone(), model);
                extractor.extract(user, assistant).await
            },
            AriesClientProvider::Azure(c) => {
                let extractor = MemoryExtractor::new(c.clone(), model);
                extractor.extract(user, assistant).await
            },
            AriesClientProvider::Deepseek(c) => {
                let extractor = MemoryExtractor::new(c.clone(), model);
                extractor.extract(user, assistant).await
            },
            AriesClientProvider::OpenAI(c) => {
                let extractor = MemoryExtractor::new(c.clone(), model);
                extractor.extract(user, assistant).await
            },
        };

        for mem in memories {
            let ExtractedMemory { name, description, memory_type, body } = mem;

            let frontmatter = MemoryFrontmatter::new(&name, &description, memory_type);
            if store.write_memory(frontmatter, body).await.is_ok() {
                let entry = ManifestEntry::new(format!("{name}.md"), description.clone());
                let _ = store.append_to_manifest(entry).await;
            }
        }
    }
}

#[derive(Clone)]
pub enum AriesAgentProvider {
    Anthropic(AriesAgent<anthropic::completion::CompletionModel<ClientWithMiddleware>>),
    Azure(AriesAgent<azure::CompletionModel<ClientWithMiddleware>>),
    Deepseek(AriesAgent<deepseek::CompletionModel<ClientWithMiddleware>>),
    OpenAI(AriesAgent<openai::CompletionModel<ClientWithMiddleware>>),
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
