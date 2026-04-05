use aries_context::GlobalContext;
use colored::Colorize;
use rig::agent::Agent;
use rig::completion::Message;
use rig::message;
use rig::message::{AssistantContent, ReasoningContent, UserContent};

use crate::AgentWrapper;

pub struct CompactionAgent<M: rig::completion::CompletionModel + 'static> {
    inner: AgentWrapper<M>,
    context: GlobalContext,
}

impl<M: rig::completion::CompletionModel + 'static> CompactionAgent<M> {
    /// 默认滑动窗口大小：保留最近的消息数量
    pub const WINDOW_SIZE: usize = 20;

    /// 默认触发压缩的 token 阈值（粗略估算：1 token ≈ 4 个字符）
    pub const TOKEN_THRESHOLD: usize = 80_000;

    pub fn new(agent: Agent<M>, context: GlobalContext) -> Self {
        let name = String::from("Complaction Agent");
        let inner = AgentWrapper::new(name, agent, context.clone());
        Self { inner, context }
    }

    pub async fn compact(&mut self, messages: Vec<Message>) -> anyhow::Result<Option<String>> {
        if !self.should_compress(&messages) {
            return Ok(None);
        }

        let theme = self.context.theme;
        println!("\n{}", theme.yellow_text("🔄 触发自动上下文压缩...").bold());

        let compacted = self.compress(&messages);
        let final_res = self.inner.completion(&compacted, vec![]).await?;

        let summary = final_res.response();
        if summary.is_empty() {
            return Ok(None);
        }

        Ok(Some(summary.to_owned()))
    }

    fn compress(&self, messages: &[Message]) -> String {
        let mut prompt = String::from("--- 对话开始 ---\n");

        for message in messages {
            match message {
                Message::System { content } => {
                    prompt.push_str(&format!("[系统]: {content}\n"));
                },
                Message::User { content } => prompt.push_str(
                    content
                        .iter()
                        .filter_map(|c| match c {
                            UserContent::Text(message::Text { text }) => Some(text.clone()),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                        .as_str(),
                ),
                Message::Assistant { content, .. } => {
                    content
                        .iter()
                        .filter_map(|c| match c {
                            AssistantContent::Text(message::Text { text }) => Some(text.clone()),
                            AssistantContent::Reasoning(message::Reasoning { content, .. }) => Some(
                                content
                                    .iter()
                                    .filter_map(|rc| match rc {
                                        ReasoningContent::Text { text, .. } => Some(text.clone()),
                                        ReasoningContent::Encrypted(s) => Some(s.clone()),
                                        ReasoningContent::Redacted { data } => Some(data.clone()),
                                        ReasoningContent::Summary(s) => Some(s.clone()),
                                        _ => None,
                                    })
                                    .collect::<Vec<_>>()
                                    .join("\n"),
                            ),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                },
            }
        }

        prompt.push_str("--- 对话结束 ---\n\n请提供简洁但全面的摘要");
        prompt
    }

    fn should_compress(&self, messages: &[Message]) -> bool {
        if messages.len() < Self::WINDOW_SIZE {
            return false;
        }

        let estimate_tokens = estimate_message_tokens(messages);
        if estimate_tokens < Self::TOKEN_THRESHOLD {
            return false;
        }

        true
    }
}

/// 估算消息列表的 token 数量
fn estimate_message_tokens(messages: &[Message]) -> usize {
    let content = format!("{:?}", messages); // 很粗糙的计算长度
    content.len() * 4 // 每条消息额外增加约 4 个 token 的格式开销
}
