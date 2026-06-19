use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Context;
use aries_filesystem::jsonl;
use regex_lite::Regex;
use rig_core::client::CompletionClient;
use rig_core::completion::Message;
use rig_core::message::{AssistantContent, ReasoningContent, UserContent};
use rig_core::{completion, message};
use tracing::info;

use crate::agents::AriesAgent;

const PREAMBLE: &str = include_str!("prompts/compact.txt");
const NAME: &str = "Archivist";
const DESCRIPTION: &str = "Summarises a conversation transcript into a structured digest.";

#[derive(Debug)]
pub enum CompactOutcome {
    Success(Vec<Message>),
    /// 上下文超长（prompt_too_long / context_length_exceeded）——同样输入再调一次也是同错，
    /// 应立刻 trip 熔断器，不要短时间内反复浪费 token。
    PromptTooLong,
    /// 网络/超时/限流等瞬时错误——不该计入熔断器失败计数，留给上层继续重试。
    Transient(String),
    /// 模型有响应但内容空、或解析后空,按一次正常失败累计。
    Empty,
}

#[derive(Clone)]
pub struct CompactAgent<M>
where
    M: completion::CompletionModel,
{
    inner: AriesAgent<M>,
    transcript_dir: PathBuf,
}

impl<M> CompactAgent<M>
where
    M: completion::CompletionModel + 'static,
{
    const COMPACTION_MAX_TURNS: usize = 1; // 强制单论,避免陷入循环

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
            .default_max_turns(Self::COMPACTION_MAX_TURNS)
            .build();

        Self { inner: AriesAgent::new(agent, NAME, PREAMBLE, None), transcript_dir }
    }

    pub async fn compact(&mut self, messages: &[Message]) -> CompactOutcome {
        info!("Compacting {} messages", messages.len());

        let file_path = match self.save_transcript(messages).await {
            Ok(p) => p,
            Err(e) => return CompactOutcome::Transient(format!("save transcript failed: {e}")),
        };

        let compressed_prompt = compress(messages);
        let final_res =
            match self.inner.prompt::<[_; 0], Message, _>(compressed_prompt, [], ()).await {
                Ok(r) => r,
                Err(e) => {
                    return if e.is_context_exceeded() {
                        CompactOutcome::PromptTooLong
                    } else {
                        CompactOutcome::Transient(e.to_string())
                    };
                },
            };

        let summary = final_res.response().trim();
        if summary.is_empty() {
            return CompactOutcome::Empty;
        }

        let formatted = extract_summary(summary);

        CompactOutcome::Success(resume_prompt(&formatted, &file_path))
    }

    async fn save_transcript(&mut self, messages: &[Message]) -> anyhow::Result<PathBuf> {
        #[rustfmt::skip]
        tokio::fs::create_dir_all(&self.transcript_dir)
            .await
            .with_context(|| format!("Failed to create transcript directory `{}`", self.transcript_dir.display()))?;

        let now = SystemTime::now();
        let ts = now.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        let file_path = self.transcript_dir.join(format!("transcript_{ts}.json"));

        jsonl::write(&file_path, messages)
            .await
            .with_context(|| format!("Failed to write transcript file {}", file_path.display()))?;

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
pub fn extract_summary(raw: &str) -> String {
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

    prompt.push_str("\n\n--- 对话结束 ---\n\n");
    prompt.push_str("请基于上面的对话生成摘要，严格遵循 <analysis>/<summary> 双标签结构。");
    prompt
}
