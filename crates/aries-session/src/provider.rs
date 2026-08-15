use std::path::Path;

use aries_agent::{AgentBuilder, AriesAgentProvider};
use aries_compact::{CompactAgent, CompactAgentProvider};
use aries_event::Notifier;
use aries_extension::AgentExtensions;
use aries_init::{GlobalContext, ModelConfig};
use aries_lspclient::SharedLspClient;
use aries_memory::{MemoryAgent, MemoryAgentProvider, MemoryRetriever, MemoryRetrieverProvider};
use aries_mode::Mode;
use http::{HeaderMap, header};
use reqwest_middleware::ClientWithMiddleware;
use reqwest_retry::RetryTransientMiddleware;
use reqwest_retry::policies::ExponentialBackoff;
use rig_agent::tool::server::ToolServerHandle;
use rig_core::http_client;
use rig_core::providers::{anthropic, azure, deepseek, openai};

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
        let mut default_headers = HeaderMap::new();
        default_headers.insert("HTTP-Referer", header::HeaderValue::from_static("")); // TODO
        default_headers.insert("X-Title", header::HeaderValue::from_static("Aries"));
        let http_client = reqwest::Client::builder()
            .default_headers(default_headers)
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
        notifier: Notifier,
    ) -> anyhow::Result<AriesAgentProvider> {
        let model = config.model();
        let cwd = cwd.as_ref().to_owned();

        match self {
            AriesClientProvider::Anthropic(c) => {
                let agent = AgentBuilder::new(c.clone(), &model, mode, cwd, gctx, notifier)
                    .with_lsp_client(lsp_client)
                    .with_extensions(extensions)
                    .build(tool_server_handle)
                    .await;
                Ok(AriesAgentProvider::Anthropic(agent))
            },
            AriesClientProvider::Azure(c) => {
                let agent = AgentBuilder::new(c.clone(), &model, mode, cwd, gctx, notifier)
                    .with_lsp_client(lsp_client)
                    .with_extensions(extensions)
                    .build(tool_server_handle)
                    .await;
                Ok(AriesAgentProvider::Azure(agent))
            },
            AriesClientProvider::Deepseek(c) => {
                let agent = AgentBuilder::new(c.clone(), &model, mode, cwd, gctx, notifier)
                    .with_lsp_client(lsp_client)
                    .with_extensions(extensions)
                    .build(tool_server_handle)
                    .await;
                Ok(AriesAgentProvider::Deepseek(agent))
            },
            AriesClientProvider::OpenAI(c) => {
                let agent = AgentBuilder::new(c.clone(), &model, mode, cwd, gctx, notifier)
                    .with_lsp_client(lsp_client)
                    .with_extensions(extensions)
                    .build(tool_server_handle)
                    .await;
                Ok(AriesAgentProvider::OpenAI(agent))
            },
        }
    }

    pub fn compact_agent(
        &self,
        model: impl Into<String>,
        transcript_path: impl AsRef<Path>,
    ) -> CompactAgentProvider {
        match self {
            AriesClientProvider::Anthropic(c) => CompactAgentProvider::Anthropic(
                CompactAgent::new(c.clone(), model, transcript_path),
            ),
            AriesClientProvider::Azure(c) => {
                CompactAgentProvider::Azure(CompactAgent::new(c.clone(), model, transcript_path))
            },
            AriesClientProvider::Deepseek(c) => {
                CompactAgentProvider::Deepseek(CompactAgent::new(c.clone(), model, transcript_path))
            },
            AriesClientProvider::OpenAI(c) => {
                CompactAgentProvider::OpenAI(CompactAgent::new(c.clone(), model, transcript_path))
            },
        }
    }

    pub async fn memory_agent(
        &self,
        model: impl Into<String>,
        mem_dir: impl AsRef<Path>,
    ) -> MemoryAgentProvider {
        match self {
            AriesClientProvider::Anthropic(c) => {
                MemoryAgentProvider::Anthropic(MemoryAgent::new(c.clone(), model, mem_dir).await)
            },
            AriesClientProvider::Azure(c) => {
                MemoryAgentProvider::Azure(MemoryAgent::new(c.clone(), model, mem_dir).await)
            },
            AriesClientProvider::Deepseek(c) => {
                MemoryAgentProvider::Deepseek(MemoryAgent::new(c.clone(), model, mem_dir).await)
            },
            AriesClientProvider::OpenAI(c) => {
                MemoryAgentProvider::OpenAI(MemoryAgent::new(c.clone(), model, mem_dir).await)
            },
        }
    }

    pub fn memory_retriever(&self, model: impl Into<String>) -> MemoryRetrieverProvider {
        match self {
            AriesClientProvider::Anthropic(c) => {
                MemoryRetrieverProvider::Anthropic(MemoryRetriever::new(c.clone(), model))
            },
            AriesClientProvider::Azure(c) => {
                MemoryRetrieverProvider::Azure(MemoryRetriever::new(c.clone(), model))
            },
            AriesClientProvider::Deepseek(c) => {
                MemoryRetrieverProvider::Deepseek(MemoryRetriever::new(c.clone(), model))
            },
            AriesClientProvider::OpenAI(c) => {
                MemoryRetrieverProvider::OpenAI(MemoryRetriever::new(c.clone(), model))
            },
        }
    }
}
