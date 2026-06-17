pub mod persistence;
pub mod registry;
pub mod session;

use std::path::Path;

use aries_core::agents::{AgentBuilder, Mode};
use aries_core::event::AgentEvent;
use aries_core::language_server::SharedLspClient;
use aries_core::{AriesResult, agents};
use aries_init::ModelConfig;
use rig_core::agent::{FinalResponse, PromptHook};
use rig_core::completion::Message;
use rig_core::providers::{azure, deepseek, openai};
use rig_core::wasm_compat::WasmCompatSend;
use tokio::sync::mpsc::UnboundedReceiver;

pub use self::persistence::{connect, migrate};
pub use self::registry::SessionRegistry;
pub use self::session::Session;

#[derive(Clone)]
pub enum AriesClient {
    Azure(azure::Client),
    Deepseek(deepseek::Client),
    OpenAI(openai::CompletionsClient),
}

impl AriesClient {
    pub fn new(config: &ModelConfig) -> anyhow::Result<Self> {
        match config {
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
    ) -> anyhow::Result<(AriesAgent, UnboundedReceiver<AgentEvent>)> {
        let model = config.model().into();
        let cwd = cwd.as_ref().to_path_buf();

        match self {
            AriesClient::Azure(c) => {
                let (agent, receiver) =
                    AgentBuilder::new(c.clone(), &model, mode, cwd).with_tools(lsp_client).await;
                Ok((AriesAgent::Azure(agent), receiver))
            },
            AriesClient::Deepseek(c) => {
                let (agent, receiver) =
                    AgentBuilder::new(c.clone(), &model, mode, cwd).with_tools(lsp_client).await;
                Ok((AriesAgent::Deepseek(agent), receiver))
            },
            AriesClient::OpenAI(c) => {
                let (agent, receiver) =
                    AgentBuilder::new(c.clone(), &model, mode, cwd).with_tools(lsp_client).await;
                Ok((AriesAgent::OpenAI(agent), receiver))
            },
        }
    }
}

#[derive(Clone)]
pub enum AriesAgent {
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
        P: PromptHook<azure::CompletionModel>
            + PromptHook<deepseek::CompletionModel>
            + PromptHook<openai::CompletionModel>
            + 'static,
    {
        match self {
            AriesAgent::Azure(a) => a.prompt(prompt, history, hook).await,
            AriesAgent::Deepseek(a) => a.prompt(prompt, history, hook).await,
            AriesAgent::OpenAI(a) => a.prompt(prompt, history, hook).await,
        }
    }

    pub fn system_prompt(&self) -> &str {
        match self {
            AriesAgent::Azure(a) => a.system_prompt(),
            AriesAgent::Deepseek(a) => a.system_prompt(),
            AriesAgent::OpenAI(a) => a.system_prompt(),
        }
    }
}
