pub mod middleware;
pub mod registry;
pub mod session;

use std::path::Path;

use aries_agent::{AgentBuilder, AriesResult};
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
use rig_core::wasm_compat::WasmCompatSend;
use tokio::sync::mpsc::UnboundedReceiver;

use crate::middleware::RetryStrategy;
pub use crate::registry::SessionRegistry;
pub use crate::session::Session;

#[derive(Clone)]
pub enum AriesProvider {
    Anthropic(anthropic::Client<ClientWithMiddleware>),
    Azure(azure::Client<ClientWithMiddleware>),
    Deepseek(deepseek::Client<ClientWithMiddleware>),
    OpenAI(openai::CompletionsClient<ClientWithMiddleware>),
}

impl AriesProvider {
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
                Ok(AriesProvider::Anthropic(client))
            },
            ModelConfig::Azure(c) => {
                let client = azure::Client::builder()
                    .api_key(&c.api_key)
                    .azure_endpoint(c.azure_endpoint.clone())
                    .api_version(&c.api_version)
                    .http_client(httpclient)
                    .build()?;
                Ok(AriesProvider::Azure(client))
            },
            ModelConfig::Deepseek(c) => {
                let client = deepseek::Client::builder()
                    .api_key(&c.api_key)
                    .http_client(httpclient)
                    .build()?;
                Ok(AriesProvider::Deepseek(client))
            },
            ModelConfig::OpenAI(c) => {
                let client = openai::CompletionsClient::builder()
                    .base_url(&c.base_url)
                    .api_key(&c.api_key)
                    .http_client(httpclient)
                    .build()?;
                Ok(AriesProvider::OpenAI(client))
            },
        }
    }

    pub async fn agent(
        &self,
        mode: Mode,
        config: ModelConfig,
        cwd: impl AsRef<Path>,
        gctx: GlobalContext,
        lsp_client: Option<SharedLspClient>,
        extensions: AgentExtensions,
        tool_server_handle: ToolServerHandle,
    ) -> anyhow::Result<(AriesAgent, UnboundedReceiver<AgentEvent>)> {
        let model = config.model();
        let cwd = cwd.as_ref().to_path_buf();

        match self {
            AriesProvider::Anthropic(c) => {
                let (agent, receiver) = AgentBuilder::new(c.clone(), &model, mode, cwd, gctx)
                    .with_lsp_client(lsp_client)
                    .with_extensions(extensions)
                    .build(tool_server_handle)
                    .await;
                Ok((AriesAgent::Anthropic(agent), receiver))
            },
            AriesProvider::Azure(c) => {
                let (agent, receiver) = AgentBuilder::new(c.clone(), &model, mode, cwd, gctx)
                    .with_lsp_client(lsp_client)
                    .with_extensions(extensions)
                    .build(tool_server_handle)
                    .await;
                Ok((AriesAgent::Azure(agent), receiver))
            },
            AriesProvider::Deepseek(c) => {
                let (agent, receiver) = AgentBuilder::new(c.clone(), &model, mode, cwd, gctx)
                    .with_lsp_client(lsp_client)
                    .with_extensions(extensions)
                    .build(tool_server_handle)
                    .await;
                Ok((AriesAgent::Deepseek(agent), receiver))
            },
            AriesProvider::OpenAI(c) => {
                let (agent, receiver) = AgentBuilder::new(c.clone(), &model, mode, cwd, gctx)
                    .with_lsp_client(lsp_client)
                    .with_extensions(extensions)
                    .build(tool_server_handle)
                    .await;
                Ok((AriesAgent::OpenAI(agent), receiver))
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
            AriesProvider::Anthropic(c) => {
                let extractor = MemoryExtractor::new(c.clone(), model);
                extractor.extract(user, assistant).await
            },
            AriesProvider::Azure(c) => {
                let extractor = MemoryExtractor::new(c.clone(), model);
                extractor.extract(user, assistant).await
            },
            AriesProvider::Deepseek(c) => {
                let extractor = MemoryExtractor::new(c.clone(), model);
                extractor.extract(user, assistant).await
            },
            AriesProvider::OpenAI(c) => {
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
pub enum AriesAgent {
    Anthropic(
        aries_agent::AriesAgent<anthropic::completion::CompletionModel<ClientWithMiddleware>>,
    ),
    Azure(aries_agent::AriesAgent<azure::CompletionModel<ClientWithMiddleware>>),
    Deepseek(aries_agent::AriesAgent<deepseek::CompletionModel<ClientWithMiddleware>>),
    OpenAI(aries_agent::AriesAgent<openai::CompletionModel<ClientWithMiddleware>>),
}

impl AriesAgent {
    #[inline]
    pub async fn prompt<I, T, P>(
        &mut self,
        prompt: impl Into<Message> + WasmCompatSend,
        history: I,
        hook: P,
    ) -> AriesResult<PromptResponse>
    where
        I: IntoIterator<Item = T>,
        T: Into<Message>,
        P: AgentHook + 'static,
    {
        match self {
            AriesAgent::Anthropic(a) => a.prompt(prompt, history, hook).await,
            AriesAgent::Azure(a) => a.prompt(prompt, history, hook).await,
            AriesAgent::Deepseek(a) => a.prompt(prompt, history, hook).await,
            AriesAgent::OpenAI(a) => a.prompt(prompt, history, hook).await,
        }
    }

    pub fn preamble(&self) -> &str {
        match self {
            AriesAgent::Anthropic(a) => a.preamble(),
            AriesAgent::Azure(a) => a.preamble(),
            AriesAgent::Deepseek(a) => a.preamble(),
            AriesAgent::OpenAI(a) => a.preamble(),
        }
    }
}
