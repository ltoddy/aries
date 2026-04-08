use aries_config::AriesConfig;
use colored::Colorize;
use rig::agent::PromptHook;
use rig::completion::Message;
use rig::message::{AssistantContent, ReasoningContent, UserContent};
use rig::providers::{azure, openai};
use rig::{completion, message};

use crate::AgentWrapper;
use crate::agent_type::AgentType;

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
    /// 默认滑动窗口大小：保留最近的消息数量
    pub const WINDOW_SIZE: usize = 20;

    /// 默认触发压缩的 token 阈值（粗略估算：4 个字符 ≈ 1 token）
    pub const TOKEN_THRESHOLD: usize = 80_000;

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

impl CompactionAgent<openai::CompletionModel, ()> {
    pub fn new(config: AriesConfig) -> anyhow::Result<Self> {
        let name = String::from("Compaction Agent");
        let inner = AgentWrapper::<openai::CompletionModel, ()>::new(name, config, AgentType::Compaction, ())?;
        Ok(Self { inner })
    }
}

impl<P> CompactionAgent<openai::CompletionModel, P>
where
    P: PromptHook<openai::CompletionModel> + 'static,
{
    pub async fn compact(&mut self, messages: Vec<Message>) -> anyhow::Result<Option<String>> {
        if !self.should_compress(&messages) {
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
            if let Some(content) = extract_message_content(message) {
                prompt.push_str("\n\n");
                prompt.push_str(&content);
            }
        }

        prompt.push_str("\n\n--- 对话结束 ---\n\n请提供简洁但全面的摘要");
        prompt
    }
}

impl CompactionAgent<azure::CompletionModel, ()> {
    pub fn new(config: AriesConfig) -> anyhow::Result<Self> {
        let name = String::from("Compaction Agent");
        let inner = AgentWrapper::<azure::CompletionModel, ()>::new(name, config, AgentType::Compaction, ())?;
        Ok(Self { inner })
    }
}

impl<P> CompactionAgent<azure::CompletionModel, P>
where
    P: PromptHook<azure::CompletionModel> + 'static,
{
    pub fn new_with_hook(config: AriesConfig, hook: P) -> anyhow::Result<Self> {
        let name = String::from("Compaction Agent");
        let inner = AgentWrapper::<azure::CompletionModel, P>::new(name, config, AgentType::Compaction, hook)?;
        Ok(Self { inner })
    }

    pub async fn compact(&mut self, messages: Vec<Message>) -> anyhow::Result<Option<String>> {
        if !self.should_compress(&messages) {
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
            if let Some(content) = extract_message_content(message) {
                prompt.push_str("\n\n");
                prompt.push_str(&content);
            }
        }

        prompt.push_str("\n\n--- 对话结束 ---\n\n请提供简洁但全面的摘要");
        prompt
    }
}

/// 估算消息列表的 token 数量
fn estimate_message_tokens(messages: &[Message]) -> usize {
    let content = format!("{:?}", messages);
    content.chars().count().div_ceil(4)
}

fn extract_message_content(message: &Message) -> Option<String> {
    let content = match message {
        Message::User { content } => content
            .iter()
            .filter_map(|c| match c {
                UserContent::Text(message::Text { text }) => Some(text.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n"),
        Message::Assistant { content, .. } => content
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
            .join("\n"),
        _ => String::new(),
    };

    if content.is_empty() { None } else { Some(content) }
}
