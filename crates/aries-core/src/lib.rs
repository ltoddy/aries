pub mod agent_type;
pub mod compaction;
pub mod display;
pub mod tools;

use std::io::Write;

use anyhow::Context;
use aries_config::AriesConfig;
use colored::Colorize;
use futures::StreamExt;
use rig::agent::{Agent, FinalResponse, MultiTurnStreamItem, PromptHook, StreamingResult, Text};
use rig::client::CompletionClient;
use rig::completion::{self, Message, Prompt};
use rig::providers::openai;
use rig::streaming::{StreamedAssistantContent, StreamedUserContent, StreamingPrompt};

use crate::agent_type::AgentType;
use crate::display::{display_token_usage, display_tool_call, display_tool_result};
pub const AGENT_LOOP_MAX_TURNS: usize = 200;

pub struct AgentWrapper {
    pub name: String,
    pub inner: Agent<openai::CompletionModel>,
}

impl AgentWrapper {
    pub fn new(name: String, config: AriesConfig, agent_type: AgentType) -> anyhow::Result<Self> {
        let client = openai::CompletionsClient::builder()
            .base_url(&config.base_url)
            .api_key(&config.api_key)
            .build()
            .with_context(|| "Failed to create llm client")?;

        let preamble = agent_type.system_prompt();
        let tools = agent_type.tools(config.clone());

        let inner = client
            .agent(&config.model)
            .preamble(preamble)
            .tools(tools)
            .default_max_turns(AGENT_LOOP_MAX_TURNS)
            .build();

        Ok(Self { name, inner })
    }

    #[inline]
    pub async fn stream_prompt<P>(
        &mut self,
        prompt: &str,
        history: &[Message],
        hook: P,
    ) -> StreamingResult<<openai::CompletionModel as completion::CompletionModel>::StreamingResponse>
    where
        P: PromptHook<openai::CompletionModel> + 'static,
    {
        self.inner.stream_prompt(prompt).with_history(history.to_vec()).with_hook(hook).await
    }

    pub async fn prompt<P>(
        &mut self,
        prompt: &str,
        history: &[Message],
        hook: P,
    ) -> anyhow::Result<String>
    where
        P: PromptHook<openai::CompletionModel> + 'static,
    {
        let res =
            self.inner.prompt(prompt).with_history(&mut history.to_vec()).with_hook(hook).await?;
        Ok(res)
    }

    pub async fn completion(
        &mut self,
        input: &str,
        history: &[Message],
    ) -> anyhow::Result<FinalResponse> {
        let theme = aries_theme::Theme::default();
        println!("{}:", theme.green_text(&self.name).bold());

        let stream = self.stream_prompt(input, history, ()).await;
        tokio::pin!(stream);
        let mut active_tools: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();
        let mut final_res = FinalResponse::empty();

        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(MultiTurnStreamItem::StreamAssistantItem(StreamedAssistantContent::Text(
                    Text { text },
                ))) => {
                    print!("{}", text);
                    let _ = std::io::stdout().flush();
                },
                Ok(MultiTurnStreamItem::StreamAssistantItem(
                    StreamedAssistantContent::Reasoning(reasoning),
                )) => {
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
                Ok(MultiTurnStreamItem::StreamAssistantItem(
                    StreamedAssistantContent::ReasoningDelta { id: _, reasoning },
                )) => {
                    print!("{}", theme.dimmed(&reasoning));
                    let _ = std::io::stdout().flush();
                },
                Ok(MultiTurnStreamItem::StreamAssistantItem(
                    StreamedAssistantContent::ToolCall { tool_call, .. },
                )) => {
                    active_tools.insert(tool_call.id.clone(), tool_call.function.name.clone());
                    display_tool_call(
                        &tool_call.function.name,
                        &tool_call.function.arguments,
                        &theme,
                    );
                },
                Ok(MultiTurnStreamItem::StreamUserItem(StreamedUserContent::ToolResult {
                    tool_result,
                    ..
                })) => {
                    let tool_name =
                        active_tools.get(&tool_result.id).cloned().unwrap_or_else(String::new);
                    let json_str =
                        serde_json::to_string(&tool_result).unwrap_or_else(|_| String::new());

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
