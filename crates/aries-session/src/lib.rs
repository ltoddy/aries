pub mod registry;
pub mod session;

use std::path::Path;

use aries_core::agents::{AgentBuilder, Mode};
use aries_core::event::AgentEvent;
use aries_core::extractor::{ExtractedMemory, MemoryExtractor};
use aries_core::{AriesResult, agents};
use aries_extension::mcp::McpConfig;
use aries_init::ModelConfig;
use aries_lspclient::SharedLspClient;
use aries_memory::{ManifestEntry, MemoryFrontmatter, MemoryStore};
pub use aries_persistence::{connect, migrate};
use rig_core::agent::{FinalResponse, PromptHook};
use rig_core::completion::Message;
use rig_core::providers::{anthropic, azure, deepseek, openai};
use rig_core::wasm_compat::WasmCompatSend;
use tokio::sync::mpsc::UnboundedReceiver;
use tracing::{info, warn};

pub use crate::registry::SessionRegistry;
pub use crate::session::Session;

#[derive(Clone)]
pub enum AriesClient {
    Anthropic(anthropic::Client),
    Azure(azure::Client),
    Deepseek(deepseek::Client),
    OpenAI(openai::CompletionsClient),
}

impl AriesClient {
    pub fn new(config: &ModelConfig) -> anyhow::Result<Self> {
        match config {
            ModelConfig::Anthropic(c) => {
                let client = anthropic::Client::builder()
                    .api_key(&c.api_key)
                    .base_url(&c.base_url)
                    .build()?;
                Ok(AriesClient::Anthropic(client))
            },
            ModelConfig::Azure(c) => {
                let client = azure::Client::builder()
                    .api_key(&c.api_key)
                    .azure_endpoint(c.azure_endpoint.clone())
                    .api_version(&c.api_version)
                    .build()?;
                Ok(AriesClient::Azure(client))
            },
            ModelConfig::Deepseek(c) => {
                let client = deepseek::Client::builder().api_key(&c.api_key).build()?;
                Ok(AriesClient::Deepseek(client))
            },
            ModelConfig::OpenAI(c) => {
                let client = openai::CompletionsClient::builder()
                    .base_url(&c.base_url)
                    .api_key(&c.api_key)
                    .build()?;
                Ok(AriesClient::OpenAI(client))
            },
        }
    }

    pub async fn agent(
        &self,
        mode: Mode,
        config: ModelConfig,
        cwd: impl AsRef<Path>,
        lsp_client: Option<SharedLspClient>,
        memory: Option<String>,
        mcp_config: McpConfig,
    ) -> anyhow::Result<(AriesAgent, UnboundedReceiver<AgentEvent>)> {
        let model = config.model();
        let cwd = cwd.as_ref().to_path_buf();

        match self {
            AriesClient::Anthropic(c) => {
                let (agent, receiver) = AgentBuilder::new(c.clone(), &model, mode, cwd)
                    .with_memory(memory)
                    .with_lsp_client(lsp_client)
                    .with_mcp_config(mcp_config)
                    .build()
                    .await;
                Ok((AriesAgent::Anthropic(agent), receiver))
            },
            AriesClient::Azure(c) => {
                let (agent, receiver) = AgentBuilder::new(c.clone(), &model, mode, cwd)
                    .with_memory(memory)
                    .with_lsp_client(lsp_client)
                    .with_mcp_config(mcp_config)
                    .build()
                    .await;
                Ok((AriesAgent::Azure(agent), receiver))
            },
            AriesClient::Deepseek(c) => {
                let (agent, receiver) = AgentBuilder::new(c.clone(), &model, mode, cwd)
                    .with_memory(memory)
                    .with_lsp_client(lsp_client)
                    .with_mcp_config(mcp_config)
                    .build()
                    .await;
                Ok((AriesAgent::Deepseek(agent), receiver))
            },
            AriesClient::OpenAI(c) => {
                let (agent, receiver) = AgentBuilder::new(c.clone(), &model, mode, cwd)
                    .with_memory(memory)
                    .with_lsp_client(lsp_client)
                    .with_mcp_config(mcp_config)
                    .build()
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
            AriesClient::Anthropic(c) => {
                let extractor = MemoryExtractor::new(c.clone(), model);
                extractor.extract(user, assistant).await
            },
            AriesClient::Azure(c) => {
                let extractor = MemoryExtractor::new(c.clone(), model);
                extractor.extract(user, assistant).await
            },
            AriesClient::Deepseek(c) => {
                let extractor = MemoryExtractor::new(c.clone(), model);
                extractor.extract(user, assistant).await
            },
            AriesClient::OpenAI(c) => {
                let extractor = MemoryExtractor::new(c.clone(), model);
                extractor.extract(user, assistant).await
            },
        };

        for mem in memories {
            let ExtractedMemory { name, description, memory_type, body } = mem;

            let frontmatter = MemoryFrontmatter::new(&name, &description, memory_type);
            match store.write_memory(frontmatter, body).await {
                Ok(_) => {
                    let entry = ManifestEntry::new(format!("{name}.md"), description.clone());
                    let _ = store.append_to_manifest(entry).await;
                    info!("memory saved: {name} (type: {memory_type:?})");
                },
                Err(e) => warn!("failed to write memory {name}: {e}"),
            }
        }
    }
}

#[derive(Clone)]
pub enum AriesAgent {
    Anthropic(agents::AriesAgent<anthropic::completion::CompletionModel>),
    Azure(agents::AriesAgent<azure::CompletionModel>),
    Deepseek(agents::AriesAgent<deepseek::CompletionModel>),
    OpenAI(agents::AriesAgent<openai::CompletionModel>),
}

impl AriesAgent {
    #[inline]
    pub async fn prompt<I, T, P>(
        &mut self,
        prompt: impl Into<Message> + WasmCompatSend,
        history: I,
        hook: P,
    ) -> AriesResult<FinalResponse>
    where
        I: IntoIterator<Item = T>,
        T: Into<Message>,
        P: PromptHook<anthropic::completion::CompletionModel>
            + PromptHook<azure::CompletionModel>
            + PromptHook<deepseek::CompletionModel>
            + PromptHook<openai::CompletionModel>
            + 'static,
    {
        match self {
            AriesAgent::Anthropic(a) => a.prompt(prompt, history, hook).await,
            AriesAgent::Azure(a) => a.prompt(prompt, history, hook).await,
            AriesAgent::Deepseek(a) => a.prompt(prompt, history, hook).await,
            AriesAgent::OpenAI(a) => a.prompt(prompt, history, hook).await,
        }
    }

    pub fn system_prompt(&self) -> &str {
        match self {
            AriesAgent::Anthropic(a) => a.system_prompt(),
            AriesAgent::Azure(a) => a.system_prompt(),
            AriesAgent::Deepseek(a) => a.system_prompt(),
            AriesAgent::OpenAI(a) => a.system_prompt(),
        }
    }
}
