use aries_config::AriesConfig;
use rig_core::providers::{azure, deepseek, openai};

use crate::error::AgentError;

pub mod agents;
pub mod compact;
pub mod error;
pub mod event;
pub mod ext;
pub mod fs;
pub mod jsonrpc;
pub mod language_server;
pub mod preamble;
pub mod repository;
pub mod tools;

pub type AriesResult<T, E = AgentError> = Result<T, E>;

#[derive(Clone)]
pub enum AriesClient {
    OpenAI(openai::CompletionsClient),
    Azure(azure::Client),
    DeepSeek(deepseek::Client),
}

pub fn create_client(config: AriesConfig) -> AriesResult<AriesClient> {
    match config {
        AriesConfig::OpenAICompatible(c) => {
            let client = openai::CompletionsClient::builder()
                .base_url(&c.base_url)
                .api_key(&c.api_key)
                .build()?;
            Ok(AriesClient::OpenAI(client))
        },
        AriesConfig::Azure(c) => {
            let client = azure::Client::builder()
                .api_key(&c.api_key)
                .azure_endpoint(c.azure_endpoint)
                .api_version(&c.api_version)
                .build()?;
            Ok(AriesClient::Azure(client))
        },
        AriesConfig::DeepSeek(c) => {
            let client = deepseek::Client::builder().api_key(&c.api_key).build()?;
            Ok(AriesClient::DeepSeek(client))
        },
    }
}
