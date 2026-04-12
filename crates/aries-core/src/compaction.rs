use anyhow::Context;
use aries_config::AriesConfig;
use colored::Colorize;
use rig::agent::PromptHook;
use rig::completion::Message;
use rig::message::{AssistantContent, ReasoningContent, UserContent};
use rig::providers::{azure, openai};
use rig::{completion, message};

use crate::{AgentType, AgentWrapper};

pub struct CompactionAgent<M, P = ()>
where
    M: completion::CompletionModel,
    P: PromptHook<M>,
{
    inner: AgentWrapper<M, P>,
}

impl<M, P> CompactionAgent<M, P>
where
    M: completion::CompletionModel + 'static,
    P: PromptHook<M> + 'static,
{
    pub const WINDOW_SIZE: usize = 20;

    pub const TOKEN_THRESHOLD: usize = 80_000;

    fn is_context_oversized(&self, messages: &[Message]) -> bool {
        messages.len() >= Self::WINDOW_SIZE && estimate_tokens_exceeds(messages, Self::TOKEN_THRESHOLD)
    }

    pub async fn compact(&mut self, messages: Vec<Message>) -> anyhow::Result<Option<String>> {
        if !self.is_context_oversized(&messages) {
            return Ok(None);
        }

        let theme = aries_theme::Theme::default();
        println!("\n{}", theme.yellow_text("🔄 触发自动上下文压缩...").bold());

        let compacted = self.compress(&messages);
        let summary = self.inner.prompt(&compacted, &[]).await?;

        if summary.is_empty() {
            return Ok(None);
        }

        Ok(Some(summary))
    }

    fn compress(&self, messages: &[Message]) -> String {
        let mut prompt = String::from("--- 对话开始 ---");

        for message in messages {
            let start_len = prompt.len();

            match message {
                Message::User { content } => {
                    for c in content.iter() {
                        if let UserContent::Text(message::Text { text }) = c {
                            if prompt.len() > start_len {
                                prompt.push('\n');
                            }
                            prompt.push_str(text);
                        }
                    }
                },
                Message::Assistant { content, .. } => {
                    for c in content.iter() {
                        match c {
                            AssistantContent::Text(message::Text { text }) => {
                                if prompt.len() > start_len {
                                    prompt.push('\n');
                                }
                                prompt.push_str(text);
                            },
                            AssistantContent::Reasoning(message::Reasoning { content, .. }) => {
                                for rc in content {
                                    let text = match rc {
                                        ReasoningContent::Text { text, .. } => text.as_str(),
                                        ReasoningContent::Encrypted(s) => s.as_str(),
                                        ReasoningContent::Redacted { data } => data.as_str(),
                                        ReasoningContent::Summary(s) => s.as_str(),
                                        _ => continue,
                                    };
                                    if prompt.len() > start_len {
                                        prompt.push('\n');
                                    }
                                    prompt.push_str(text);
                                }
                            },
                            _ => {},
                        }
                    }
                },
                _ => {},
            }

            if prompt.len() > start_len {
                prompt.insert_str(start_len, "\n\n");
            }
        }

        prompt.push_str("\n\n--- 对话结束 ---\n\n请提供简洁但全面的摘要");
        prompt
    }
}

impl CompactionAgent<openai::CompletionModel, ()> {
    pub fn new(config: AriesConfig) -> anyhow::Result<Self> {
        let AriesConfig::OpenAICompatible(ref conf) = config else {
            anyhow::bail!("OpenAI compatible agent requires an OpenAI compatible config");
        };

        let client = openai::CompletionsClient::builder()
            .base_url(&conf.base_url)
            .api_key(&conf.api_key)
            .build()
            .with_context(|| "Failed to create llm client")?;

        let name = String::from("Compaction Agent");
        let inner = AgentWrapper::new(client, name, config, AgentType::Compaction, ());
        Ok(Self { inner })
    }
}

impl CompactionAgent<azure::CompletionModel, ()> {
    pub fn new(config: AriesConfig) -> anyhow::Result<Self> {
        let AriesConfig::Azure(ref conf) = config else {
            anyhow::bail!("Azure agent requires an Azure config");
        };

        let client = azure::Client::builder()
            .api_key(&conf.api_key)
            .azure_endpoint(conf.azure_endpoint.to_owned())
            .api_version(&conf.api_version)
            .build()
            .with_context(|| "Failed to create llm client")?;

        let name = String::from("Compaction Agent");
        let inner = AgentWrapper::new(client, name, config, AgentType::Compaction, ());
        Ok(Self { inner })
    }
}

fn estimate_tokens_exceeds(messages: &[Message], threshold: usize) -> bool {
    let mut total_chars: usize = 0;

    for message in messages {
        match message {
            Message::User { content } => {
                for c in content.iter() {
                    if let UserContent::Text(message::Text { text }) = c {
                        total_chars += text.chars().count();
                    }
                }
            },
            Message::Assistant { content, .. } => {
                for c in content.iter() {
                    match c {
                        AssistantContent::Text(message::Text { text }) => total_chars += text.chars().count(),
                        AssistantContent::Reasoning(message::Reasoning { content, .. }) => {
                            for rc in content {
                                total_chars += match rc {
                                    ReasoningContent::Text { text, .. } => text.chars().count(),
                                    ReasoningContent::Encrypted(s) => s.chars().count(),
                                    ReasoningContent::Redacted { data } => data.chars().count(),
                                    ReasoningContent::Summary(s) => s.chars().count(),
                                    _ => 0,
                                };
                            }
                        },
                        _ => {},
                    }
                }
            },
            _ => {},
        }

        if total_chars.div_ceil(4) >= threshold {
            return true;
        }
    }

    false
}
