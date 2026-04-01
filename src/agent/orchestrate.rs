use std::io::Write;

use anyhow::Result;
use colored::Colorize;
use futures::StreamExt;
use rig::agent::{Agent, MultiTurnStreamItem};
use rig::completion::Message;
use rig::message::Text;
use rig::streaming::{StreamedAssistantContent, StreamedUserContent, StreamingPrompt};

use crate::agent::display::{display_token_usage, display_tool_call, display_tool_result};
use crate::context::GlobalContext;

pub struct Orchestrate<M: rig::completion::CompletionModel + 'static> {
    agent: Agent<M>,
    chat_history: Vec<Message>,
    agent_name: String,
    context: GlobalContext,
}

impl<M: rig::completion::CompletionModel + 'static> Orchestrate<M> {
    pub fn new(agent: Agent<M>, agent_name: impl Into<String>, context: GlobalContext) -> Self {
        Self { agent, chat_history: Vec::new(), agent_name: agent_name.into(), context }
    }

    pub fn set_current_dir(&mut self) {
        let prompt = format!("System info: The current working directory is {}.", self.context.current_dir.display());
        self.chat_history.push(Message::user(prompt));
    }

    pub fn clear_history(&mut self) {
        if !self.chat_history.is_empty() {
            self.chat_history.truncate(1);
        }
    }

    pub async fn completion(&mut self, input: &str) -> Result<String> {
        let mut stream = self.agent.stream_prompt(input).with_history(self.chat_history.clone()).await;

        print!("{}: ", self.agent_name.green().bold());
        let mut full_response = String::new();
        let mut active_tools: std::collections::HashMap<String, String> = std::collections::HashMap::new();

        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(MultiTurnStreamItem::StreamAssistantItem(StreamedAssistantContent::Text(Text { text }))) => {
                    print!("{}", text);
                    let _ = std::io::stdout().flush();
                    full_response.push_str(&text);
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
                    print!("{}", text.dimmed());
                    let _ = std::io::stdout().flush();
                    full_response.push_str(&text);
                },
                Ok(MultiTurnStreamItem::StreamAssistantItem(StreamedAssistantContent::ReasoningDelta {
                    id: _,
                    reasoning,
                })) => {
                    print!("{}", reasoning.dimmed());
                    let _ = std::io::stdout().flush();
                    full_response.push_str(&reasoning);
                },
                Ok(MultiTurnStreamItem::StreamAssistantItem(StreamedAssistantContent::ToolCall {
                    tool_call, ..
                })) => {
                    active_tools.insert(tool_call.id.clone(), tool_call.function.name.clone());
                    display_tool_call(&tool_call.function.name, &tool_call.function.arguments);
                },
                Ok(MultiTurnStreamItem::StreamUserItem(StreamedUserContent::ToolResult { tool_result, .. })) => {
                    let tool_name = active_tools.get(&tool_result.id).cloned().unwrap_or_else(String::new);
                    let json_str = serde_json::to_string(&tool_result).unwrap_or_else(|_| String::new());

                    display_tool_result(&tool_name, &json_str);
                },
                Ok(MultiTurnStreamItem::FinalResponse(res)) => {
                    if let Some(history) = res.history() {
                        self.chat_history = history.to_vec();
                    }
                    display_token_usage(&res.usage());
                },
                Err(e) => eprintln!("\n{}: {}", "Error streaming chunk".red(), e),
                _ => {},
            }
        }
        println!();
        Ok(full_response)
    }
}
