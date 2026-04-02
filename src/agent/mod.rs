pub mod agent_type;
pub mod display;
pub mod orchestrate;

pub use agent_type::AgentType;
use rig::agent::Agent;
use rig::client::CompletionClient;
use rig::providers::openai::CompletionsClient;
use rig::providers::openai::completion::CompletionModel;

use crate::context::GlobalContext;

pub fn create(context: &GlobalContext, agent_type: AgentType) -> anyhow::Result<Agent<CompletionModel>> {
    let client = CompletionsClient::builder()
        .api_key(context.config.api_key.clone())
        .base_url(context.config.base_url.clone())
        .build()
        .map_err(|e| anyhow::anyhow!(e))?;

    let preamble = agent_type.system_prompt();
    let tools = agent_type.tools(context);

    Ok(client.agent(&context.config.model_name).preamble(preamble).tools(tools).default_max_turns(200).build())
}
