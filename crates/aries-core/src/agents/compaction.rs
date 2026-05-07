use std::collections::{HashMap, HashSet};
use std::path::Path;

use rig::client::CompletionClient;
use rig::completion::Message;
use rig::message::{AssistantContent, ReasoningContent, ToolResultContent, UserContent};
use rig::one_or_many::OneOrMany;
use rig::{completion, message};

use crate::agents::{AGENT_LOOP_MAX_TURNS, AriesAgent};

const KEEP_RECENT_TOOL_RESULTS: usize = 3;

const PREAMBLE: &str = include_str!("prompts/compaction.txt");
const NAME: &str = "Archivist";
const DESCRIPTION: &str = "用于压缩和总结对话上下文的智能体。";

#[derive(Clone)]
pub struct CompactionAgent<M>
where
    M: completion::CompletionModel,
{
    inner: AriesAgent<M>,
}

impl<M> CompactionAgent<M>
where
    M: completion::CompletionModel + 'static,
{
    pub const TOKEN_THRESHOLD: usize = 80_000;

    pub fn new<C>(client: C, model: &str) -> Self
    where
        C: CompletionClient<CompletionModel = M>,
    {
        let agent = client
            .agent(model)
            .name(NAME)
            .description(DESCRIPTION)
            .preamble(PREAMBLE)
            .default_max_turns(AGENT_LOOP_MAX_TURNS)
            .build();
        Self { inner: AriesAgent::new(agent, PREAMBLE.to_owned()) }
    }

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
        transcript_dir: impl AsRef<Path>,
    ) -> anyhow::Result<Option<Vec<Message>>> {
        if !estimate_tokens_exceeds(messages, Self::TOKEN_THRESHOLD) {
            return Ok(None);
        }

        self.compact_inner(messages, transcript_dir.as_ref()).await
    }

    pub async fn force_compact(
        &mut self,
        messages: &[Message],
        transcript_dir: impl AsRef<Path>,
    ) -> anyhow::Result<Option<Vec<Message>>> {
        self.compact_inner(messages, transcript_dir.as_ref()).await
    }

    async fn compact_inner(
        &mut self,
        messages: &[Message],
        transcript_dir: impl AsRef<Path>,
    ) -> anyhow::Result<Option<Vec<Message>>> {
        println!("\n🔄 触发上下文压缩...");

        save_transcript(messages, transcript_dir.as_ref()).await?;

        let compacted = compress(messages);
        let summary = self.inner.complete(&compacted, &[]).await?;

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

async fn save_transcript(
    messages: &[Message],
    transcript_dir: impl AsRef<Path>,
) -> anyhow::Result<()> {
    let transcript_dir = transcript_dir.as_ref();
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
