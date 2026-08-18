use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Context;
use aries_event::Notifier;
use aries_filesystem::jsonl;
use futures::StreamExt;
use regex_lite::Regex;
use rig::Agent;
use rig::agent::{MultiTurnStreamItem, PromptResponse, StreamingError};
use rig::client::AgentClientExt;
use rig::completion::{CompletionError, Message};
use rig::message::{self, AssistantContent, ReasoningContent, UserContent};
use rig::streaming::StreamingPrompt;
use tokio::pin;

const PREAMBLE: &str = include_str!("preamble.md");
const NAME: &str = "Archivist";
const DESCRIPTION: &str = "Summarises a conversation transcript into a structured digest.";

#[derive(Debug)]
pub enum CompactOutcome {
    Success((Vec<Message>, String)),
    /// 上下文超长（prompt_too_long / context_length_exceeded）——同样输入再调一次也是同错，
    /// 应立刻 trip 熔断器，不要短时间内反复浪费 token。
    PromptTooLong,
    /// 网络/超时/限流等瞬时错误——不该计入熔断器失败计数，留给上层继续重试。
    Transient(String),
    /// 模型有响应但内容空、或解析后空,按一次正常失败累计。
    Empty,
}

#[derive(Clone)]
pub struct CompactAgent {
    inner: Agent,
    transcript_path: PathBuf,
    notifier: Notifier,
}

impl CompactAgent {
    const COMPACTION_MAX_TURNS: usize = 1; // 强制单论,避免陷入循环

    pub fn new<C>(
        c: C,
        model: impl Into<String>,
        transcript_path: impl AsRef<Path>,
        notifier: Notifier,
    ) -> Self
    where
        C: AgentClientExt + 'static,
    {
        let transcript_path = transcript_path.as_ref().to_path_buf();

        let agent = c
            .agent(model)
            .name(NAME)
            .description(DESCRIPTION)
            .preamble(PREAMBLE)
            .default_max_turns(Self::COMPACTION_MAX_TURNS)
            .build();

        Self { inner: agent, transcript_path, notifier }
    }

    pub async fn compact(&mut self, messages: &[Message]) -> CompactOutcome {
        let file_path = match self.save_transcript(messages).await {
            Ok(p) => p,
            Err(e) => return CompactOutcome::Transient(format!("save transcript failed: {e}")),
        };

        let compressed_prompt = compress(messages);
        let stream = self.inner.stream_prompt(compressed_prompt).await;
        pin!(stream);

        let mut final_res = PromptResponse::empty();
        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(chunk) => {
                    self.notifier.send_stream_item(chunk.clone());
                    if let MultiTurnStreamItem::FinalResponse(res) = chunk {
                        final_res = res;
                    }
                },
                Err(err) => {
                    if let StreamingError::Completion(CompletionError::ProviderError(ref err)) = err
                    {
                        const PATTERNS: [&str; 6] = [
                            "prompt_too_long",
                            "context_length_exceeded",
                            "maximum context length",
                            "context length exceeded",
                            "too many tokens",
                            "input is too long",
                        ];
                        if PATTERNS.iter().any(|p| err.contains(p)) {
                            return CompactOutcome::PromptTooLong;
                        }
                    }
                    return CompactOutcome::Transient(err.to_string());
                },
            }
        }

        let summary = final_res.output().trim();
        if summary.is_empty() {
            return CompactOutcome::Empty;
        }

        let compact_summary = compact_summary(summary);

        CompactOutcome::Success((resume_prompt(&compact_summary, &file_path), compact_summary))
    }

    async fn save_transcript(&mut self, messages: &[Message]) -> anyhow::Result<PathBuf> {
        #[rustfmt::skip]
        tokio::fs::create_dir_all(&self.transcript_path)
            .await
            .with_context(|| format!("failed to create transcript directory `{}`", self.transcript_path.display()))?;

        let now = SystemTime::now();
        let ts = now.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        let file_path = self.transcript_path.join(format!("transcript_{ts}.json"));

        jsonl::write(&file_path, messages)
            .await
            .with_context(|| format!("failed to write transcript file {}", file_path.display()))?;

        Ok(file_path)
    }
}

fn resume_prompt(formatted_summary: &str, transcript_path: impl AsRef<Path>) -> Vec<Message> {
    let transcript_path = transcript_path.as_ref();

    let summary = format!(
        "本次会话承接此前一段已超出上下文窗口的对话。下面是早前对话的摘要。\n\n{formatted_summary}"
    );
    let hint = format!(
        r#"如果你需要压缩前的具体细节（例如完整代码片段、错误信息或你之前生成的内容），可以读取完整 transcript：{}。请直接从中断的位置继续，不要再向用户提任何确认问题。直接续做——不要复述摘要、不要回顾刚才在做什么、不要以"我将继续"之类的开场白起头，就当中断从未发生过那样接着完成最后那项任务。"#,
        transcript_path.display()
    );

    vec![Message::user(summary), Message::user(hint), Message::assistant("好的，继续。")]
}

/// 从模型输出中提取 `<summary>` 块内容，格式化为 "Summary:\n..."。
pub fn compact_summary(raw: &str) -> String {
    let re = Regex::new(r"(?s)<summary>(.*?)</summary>").unwrap();

    re.captures(raw)
        .map(|caps| format!("Summary:\n{}", caps[1].trim()))
        .unwrap_or_else(|| raw.trim().to_owned())
}

fn compress(messages: &[Message]) -> String {
    let mut prompt = String::from("--- 对话开始 ---");

    for message in messages {
        let start_len = prompt.len();

        match message {
            Message::User { content } => {
                for c in content.iter() {
                    if let UserContent::Text(t) = c {
                        if prompt.len() > start_len {
                            prompt.push('\n');
                        }
                        prompt.push_str(t.text());
                    }
                }
            },
            Message::Assistant { content, .. } => {
                for c in content.iter() {
                    match c {
                        AssistantContent::Text(t) => {
                            if prompt.len() > start_len {
                                prompt.push('\n');
                            }
                            prompt.push_str(t.text());
                        },
                        AssistantContent::Reasoning(message::Reasoning { content, .. }) => {
                            for rc in content {
                                let text = match rc {
                                    ReasoningContent::Text { text, .. } => text.as_str(),
                                    ReasoningContent::Encrypted(s) => s.as_str(),
                                    ReasoningContent::Redacted { data } => data.as_str(),
                                    ReasoningContent::Summary(s) => s.as_str(),
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

    prompt.push_str("\n\n--- 对话结束 ---\n\n");
    prompt.push_str("请基于上面的对话生成摘要，严格遵循 <analysis>/<summary> 双标签结构。");
    prompt
}
