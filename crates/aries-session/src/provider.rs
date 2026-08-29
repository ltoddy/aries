use std::path::Path;

use aries_agent::{AgentBuilder, AriesAgent};
use aries_compact::CompactAgent;
use aries_event::Notifier;
use aries_extension::AgentExtensions;
use aries_init::{GlobalContext, ModelConfig};
use aries_lspclient::SharedLspClient;
use aries_memory::{MemoryAgent, MemoryRetriever};
use aries_mode::Mode;
use http::{HeaderMap, header};
use reqwest_middleware::ClientWithMiddleware;
use reqwest_retry::RetryTransientMiddleware;
use reqwest_retry::policies::ExponentialBackoff;
use rig::agent::ModelHandle;
use rig::client::CompletionClient;
use rig::http_client;
use rig::providers::{anthropic, azure, deepseek, openai};
use rig::tool::server::ToolServerHandle;

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

    pub fn completion_model(&self, model: impl Into<String>) -> ModelHandle {
        let model = model.into();
        match self {
            AriesClientProvider::Anthropic(c) => {
                ModelHandle::new(c.completion_model(model.clone()))
            },
            AriesClientProvider::Azure(c) => ModelHandle::new(c.completion_model(model.clone())),
            AriesClientProvider::Deepseek(c) => ModelHandle::new(c.completion_model(model.clone())),
            AriesClientProvider::OpenAI(c) => ModelHandle::new(c.completion_model(model)),
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
    ) -> anyhow::Result<AriesAgent> {
        let model = config.model();
        let cwd = cwd.as_ref().to_owned();

        match self {
            AriesClientProvider::Anthropic(c) => {
                let agent = AgentBuilder::new(c.clone(), &model, mode, cwd, gctx, notifier)
                    .with_lsp_client(lsp_client)
                    .with_extensions(extensions)
                    .build(tool_server_handle)
                    .await;
                Ok(agent)
            },
            AriesClientProvider::Azure(c) => {
                let agent = AgentBuilder::new(c.clone(), &model, mode, cwd, gctx, notifier)
                    .with_lsp_client(lsp_client)
                    .with_extensions(extensions)
                    .build(tool_server_handle)
                    .await;
                Ok(agent)
            },
            AriesClientProvider::Deepseek(c) => {
                let agent = AgentBuilder::new(c.clone(), &model, mode, cwd, gctx, notifier)
                    .with_lsp_client(lsp_client)
                    .with_extensions(extensions)
                    .build(tool_server_handle)
                    .await;
                Ok(agent)
            },
            AriesClientProvider::OpenAI(c) => {
                let agent = AgentBuilder::new(c.clone(), &model, mode, cwd, gctx, notifier)
                    .with_lsp_client(lsp_client)
                    .with_extensions(extensions)
                    .build(tool_server_handle)
                    .await;
                Ok(agent)
            },
        }
    }

    pub fn compact_agent(
        &self,
        model: impl Into<String>,
        transcript_path: impl AsRef<Path>,
        notifier: Notifier,
    ) -> CompactAgent {
        match self {
            AriesClientProvider::Anthropic(c) => {
                CompactAgent::new(c.clone(), model, transcript_path, notifier)
            },
            AriesClientProvider::Azure(c) => {
                CompactAgent::new(c.clone(), model, transcript_path, notifier)
            },
            AriesClientProvider::Deepseek(c) => {
                CompactAgent::new(c.clone(), model, transcript_path, notifier)
            },
            AriesClientProvider::OpenAI(c) => {
                CompactAgent::new(c.clone(), model, transcript_path, notifier)
            },
        }
    }

    pub async fn memory_agent(
        &self,
        model: impl Into<String>,
        mem_dir: impl AsRef<Path>,
        notifier: Notifier,
    ) -> MemoryAgent {
        match self {
            AriesClientProvider::Anthropic(c) => {
                MemoryAgent::new(c.clone(), model, mem_dir, notifier).await
            },
            AriesClientProvider::Azure(c) => {
                MemoryAgent::new(c.clone(), model, mem_dir, notifier).await
            },
            AriesClientProvider::Deepseek(c) => {
                MemoryAgent::new(c.clone(), model, mem_dir, notifier).await
            },
            AriesClientProvider::OpenAI(c) => {
                MemoryAgent::new(c.clone(), model, mem_dir, notifier).await
            },
        }
    }

    pub fn memory_retriever(&self, model: impl Into<String>) -> MemoryRetriever {
        match self {
            AriesClientProvider::Anthropic(c) => MemoryRetriever::new(c.clone(), model),
            AriesClientProvider::Azure(c) => MemoryRetriever::new(c.clone(), model),
            AriesClientProvider::Deepseek(c) => MemoryRetriever::new(c.clone(), model),
            AriesClientProvider::OpenAI(c) => MemoryRetriever::new(c.clone(), model),
        }
    }
}
