pub mod agent_type;
pub mod display;
pub mod orchestrate;

pub use agent_type::AgentType;
use rig::client::Client;
use rig::providers::openai;

use crate::config::AppConfig;

pub fn create_client(config: &AppConfig) -> anyhow::Result<Client<openai::OpenAICompletionsExt>> {
    let client = openai::Client::builder()
        .api_key(config.api_key.clone())
        .base_url(config.base_url.clone())
        .build()
        .map_err(|e| anyhow::anyhow!(e))?
        .completions_api();

    Ok(client)
}
