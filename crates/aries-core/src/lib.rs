pub mod agent_type;
pub mod compaction;
pub mod display;
pub mod orchestrate;
pub mod tools;

use std::io::Write;

use aries_context::GlobalContext;
use colored::Colorize;
use futures::StreamExt;
use rig::agent::{Agent, FinalResponse, MultiTurnStreamItem, Text};
use rig::client::CompletionClient;
use rig::completion::Message;
use rig::providers::openai::CompletionsClient;
use rig::providers::openai::completion::CompletionModel;
use rig::streaming::{StreamedAssistantContent, StreamedUserContent, StreamingPrompt};

pub use crate::agent_type::AgentType;
use crate::display::{display_token_usage, display_tool_call, display_tool_result};

pub fn create(gctx: GlobalContext, agent_type: AgentType) -> anyhow::Result<Agent<CompletionModel>> {
    let api_key = &gctx.config.api_key;
    let base_url = &gctx.config.base_url;
    let model = &gctx.config.model;

    let client =
        CompletionsClient::builder().api_key(api_key).base_url(base_url).build().map_err(|e| anyhow::anyhow!(e))?;

    let preamble = agent_type.system_prompt();
    let tools = agent_type.tools(gctx.clone());

    Ok(client.agent(model).preamble(preamble).tools(tools).default_max_turns(200).build())
}

pub struct AgentWrapper<M: rig::completion::CompletionModel + 'static> {
    name: String,
    agent: Agent<M>,
}

impl<M> AgentWrapper<M>
where
    M: rig::completion::CompletionModel + 'static,
{
    pub fn new(name: String, agent: Agent<M>) -> Self {
        Self { name, agent }
    }

    pub async fn completion(&mut self, input: &str, history: Vec<Message>) -> anyhow::Result<FinalResponse> {
        let theme = aries_context::Theme::default();
        println!("{}:", theme.green_text(&self.name).bold());

        let mut stream = self.agent.stream_prompt(input).with_history(history).await;
        let mut active_tools: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        let mut final_res = FinalResponse::empty();

        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(MultiTurnStreamItem::StreamAssistantItem(StreamedAssistantContent::Text(Text { text }))) => {
                    print!("{}", text);
                    let _ = std::io::stdout().flush();
                },
                Ok(MultiTurnStreamItem::StreamAssistantItem(StreamedAssistantContent::Reasoning(reasoning))) => {
                    let text = reasoning
                        .content
                        .iter()
                        .map(|c| match c {
                            rig::message::ReasoningContent::Text { text, .. } => text.clone(),
                            rig::message::ReasoningContent::Encrypted(s) => s.clone(),
                            rig::message::ReasoningContent::Redacted { data } => data.clone(),
                            rig::message::ReasoningContent::Summary(s) => s.clone(),
                            _ => String::new(),
                        })
                        .collect::<String>();
                    print!("{}", theme.dimmed(&text));
                    let _ = std::io::stdout().flush();
                },
                Ok(MultiTurnStreamItem::StreamAssistantItem(StreamedAssistantContent::ReasoningDelta {
                    id: _,
                    reasoning,
                })) => {
                    print!("{}", theme.dimmed(&reasoning));
                    let _ = std::io::stdout().flush();
                },
                Ok(MultiTurnStreamItem::StreamAssistantItem(StreamedAssistantContent::ToolCall {
                    tool_call, ..
                })) => {
                    active_tools.insert(tool_call.id.clone(), tool_call.function.name.clone());
                    display_tool_call(&tool_call.function.name, &tool_call.function.arguments, &theme);
                },
                Ok(MultiTurnStreamItem::StreamUserItem(StreamedUserContent::ToolResult { tool_result, .. })) => {
                    let tool_name = active_tools.get(&tool_result.id).cloned().unwrap_or_else(String::new);
                    let json_str = serde_json::to_string(&tool_result).unwrap_or_else(|_| String::new());

                    display_tool_result(&tool_name, &json_str, &theme);
                },
                Ok(MultiTurnStreamItem::FinalResponse(res)) => {
                    display_token_usage(&res.usage(), &theme);
                    final_res = res
                },
                Err(e) => eprintln!("\n{}: {}", theme.red_text("Error streaming_chunk"), e),
                Ok(_) => {},
            }
        }
        println!();
        Ok(final_res)
    }
}
