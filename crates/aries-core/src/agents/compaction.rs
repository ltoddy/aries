use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Context;
use rig_core::client::CompletionClient;
use rig_core::completion::Message;
use rig_core::message::{AssistantContent, ReasoningContent, UserContent};
use rig_core::{completion, message};
use tracing::info;

use crate::agents::{AGENT_LOOP_MAX_TURNS, AriesAgent};

const PREAMBLE: &str = include_str!("prompts/compaction.txt");
const NAME: &str = "Archivist";
const DESCRIPTION: &str = "用于压缩和总结对话上下文的智能体。";

#[derive(Clone)]
pub struct CompactionAgent<M>
where
    M: completion::CompletionModel,
{
    inner: AriesAgent<M>,
    transcript_dir: PathBuf,
}

impl<M> CompactionAgent<M>
where
    M: completion::CompletionModel + 'static,
{
    pub fn new<C>(c: C, model: impl Into<String>, transcript_dir: impl AsRef<Path>) -> Self
    where
        C: CompletionClient<CompletionModel = M> + 'static,
    {
        let transcript_dir = transcript_dir.as_ref().to_path_buf();

        let agent = c
            .agent(model)
            .name(NAME)
            .description(DESCRIPTION)
            .preamble(PREAMBLE)
            .default_max_turns(AGENT_LOOP_MAX_TURNS)
            .build();

        Self { inner: AriesAgent::new(agent, NAME, PREAMBLE, None), transcript_dir }
    }

    pub async fn compact(&mut self, messages: &[Message]) -> Option<Vec<Message>> {
        info!("触发上下文压缩");

        let file_path = self.save_transcript(messages).await.ok()?;
        let compacted = compress(messages);
        let final_res = self.inner.prompt::<Vec<_>, Message>(&compacted, vec![]).await.ok()?;
        let summary = final_res.response().to_owned();
        if summary.is_empty() {
            return None;
        }

        let compressed_messages = vec![
            Message::user(format!("[Compressed]\n\n{}", summary)),
            Message::user(format!(
                "The full conversation transcript has been saved to: {}",
                file_path.display()
            )),
            Message::assistant("Understood. Continuing."),
        ];

        Some(compressed_messages)
    }

    async fn save_transcript(&mut self, messages: &[Message]) -> anyhow::Result<PathBuf> {
        if !self.transcript_dir.exists() {
            tokio::fs::create_dir_all(&self.transcript_dir).await.with_context(|| {
                format!("Failed to create transcript directory `{}`", self.transcript_dir.display())
            })?
        }

        let now = SystemTime::now();
        let timestamp = now.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();

        let file_path = self.transcript_dir.join(format!("transcript_{timestamp}.json"));

        let content = serde_json::to_string(messages)
            .with_context(|| "Failed to serialize transcript messages")?;

        tokio::fs::write(&file_path, &content)
            .await
            .with_context(|| format!("Failed to write transcript file {}", file_path.display()))?;

        Ok(file_path)
    }
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
