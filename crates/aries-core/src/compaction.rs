use std::collections::{HashMap, HashSet};
use std::path::Path;

use anyhow::Context;
use aries_config::AriesConfig;
use colored::Colorize;
use rig::agent::PromptHook;
use rig::completion::Message;
use rig::message::{AssistantContent, ReasoningContent, ToolResultContent, UserContent};
use rig::one_or_many::OneOrMany;
use rig::providers::{azure, openai};
use rig::{completion, message};

use crate::{AgentType, AgentWrapper};

const KEEP_RECENT_TOOL_RESULTS: usize = 3;

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
    pub const TOKEN_THRESHOLD: usize = 80_000;

    pub fn micro_compact(messages: &mut [Message]) {
        let tool_name_map = build_tool_name_map(messages);

        let mut tool_result_ids: Vec<String> = Vec::new();
        for msg in messages.iter() {
            if let Message::User { content } = msg {
                for part in content.iter() {
                    if let UserContent::ToolResult(tr) = part {
                        tool_result_ids.push(tr.id.clone());
                    }
                }
            }
        }

        if tool_result_ids.len() <= KEEP_RECENT_TOOL_RESULTS {
            return;
        }

        let to_replace_ids: HashSet<&str> = tool_result_ids
            [..tool_result_ids.len() - KEEP_RECENT_TOOL_RESULTS]
            .iter()
            .map(|s| s.as_str())
            .collect();

        for msg in messages.iter_mut() {
            if let Message::User { content } = msg {
                for item in content.iter_mut() {
                    if let UserContent::ToolResult(tr) = item {
                        if !to_replace_ids.contains(tr.id.as_str()) {
                            continue;
                        }

                        let text_len: usize = tr
                            .content
                            .iter()
                            .map(|c| match c {
                                ToolResultContent::Text(t) => t.text.len(),
                                _ => 0,
                            })
                            .sum();

                        if text_len <= 100 {
                            continue;
                        }

                        let tool_name =
                            tool_name_map.get(&tr.id).map(|s| s.as_str()).unwrap_or("unknown");

                        let placeholder = format!("[Previous: used {}]", tool_name);
                        *item = UserContent::tool_result(
                            tr.id.clone(),
                            OneOrMany::one(ToolResultContent::text(placeholder)),
                        );
                    }
                }
            }
        }
    }

    pub async fn auto_compact(
        &mut self,
        messages: &[Message],
        transcript_dir: &Path,
    ) -> anyhow::Result<Option<Vec<Message>>> {
        if !estimate_tokens_exceeds(messages, Self::TOKEN_THRESHOLD) {
            return Ok(None);
        }

        self.compact_inner(messages, transcript_dir).await
    }

    pub async fn force_compact(
        &mut self,
        messages: &[Message],
        transcript_dir: &Path,
    ) -> anyhow::Result<Option<Vec<Message>>> {
        self.compact_inner(messages, transcript_dir).await
    }

    async fn compact_inner(
        &mut self,
        messages: &[Message],
        transcript_dir: &Path,
    ) -> anyhow::Result<Option<Vec<Message>>> {
        let theme = aries_theme::Theme::default();
        println!("\n{}", theme.yellow_text("🔄 触发上下文压缩...").bold());

        save_transcript(messages, transcript_dir).await?;

        let compacted = compress(messages);
        let summary = self.inner.prompt(&compacted, &[]).await?;

        if summary.is_empty() {
            return Ok(None);
        }

        let compressed_messages = vec![
            Message::user(format!("[Compressed]\n\n{}", summary)),
            Message::assistant("Understood. Continuing."),
        ];

        Ok(Some(compressed_messages))
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

        let inner = AgentWrapper::new(client, config, AgentType::Compaction, ()).build();
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

        let inner = AgentWrapper::new(client, config, AgentType::Compaction, ()).build();
        Ok(Self { inner })
    }
}

fn build_tool_name_map(messages: &[Message]) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for msg in messages {
        if let Message::Assistant { content, .. } = msg {
            for c in content.iter() {
                if let AssistantContent::ToolCall(tc) = c {
                    map.insert(tc.id.clone(), tc.function.name.clone());
                }
            }
        }
    }
    map
}

async fn save_transcript(messages: &[Message], transcript_dir: &Path) -> anyhow::Result<()> {
    tokio::fs::create_dir_all(transcript_dir).await?;

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let path = transcript_dir.join(format!("transcript_{}.json", timestamp));

    let content = serde_json::to_string_pretty(messages)?;
    tokio::fs::write(path, content).await?;

    Ok(())
}

fn compress(messages: &[Message]) -> String {
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
                        AssistantContent::Text(message::Text { text }) => {
                            total_chars += text.chars().count()
                        },
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
