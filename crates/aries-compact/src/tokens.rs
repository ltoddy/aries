use rig_core::OneOrMany;
use rig_core::message::{
    AssistantContent, Message, ReasoningContent, ToolResultContent, UserContent,
};

pub trait TokenEstimator {
    fn estimate_tokens(&self) -> u64;
}

impl TokenEstimator for &str {
    fn estimate_tokens(&self) -> u64 {
        if self.is_empty() {
            return 0;
        }
        (self.chars().count() as u64).div_ceil(CHARS_PER_TOKEN).max(1)
    }
}

impl TokenEstimator for String {
    fn estimate_tokens(&self) -> u64 {
        if self.is_empty() {
            return 0;
        }
        (self.chars().count() as u64).div_ceil(CHARS_PER_TOKEN).max(1)
    }
}

impl TokenEstimator for &[Message] {
    fn estimate_tokens(&self) -> u64 {
        self.iter()
            .map(|m| m.estimate_tokens())
            .sum::<u64>()
            .saturating_mul(CONSERVATIVE_NUM)
            .div_ceil(CONSERVATIVE_DEN)
    }
}

impl TokenEstimator for Message {
    fn estimate_tokens(&self) -> u64 {
        match self {
            Message::System { content } => content.estimate_tokens(),
            Message::User { content } => content.estimate_tokens(),
            Message::Assistant { content, .. } => content.estimate_tokens(),
        }
    }
}

impl TokenEstimator for OneOrMany<UserContent> {
    fn estimate_tokens(&self) -> u64 {
        self.iter().map(|content| content.estimate_tokens()).sum::<u64>()
    }
}

impl TokenEstimator for UserContent {
    fn estimate_tokens(&self) -> u64 {
        match self {
            UserContent::Text(t) => t.text.estimate_tokens(),
            UserContent::ToolResult(tr) => tr.content.estimate_tokens(),
            UserContent::Image(_) => IMAGE_MAX_TOKEN_SIZE,
            UserContent::Audio(_) => IMAGE_MAX_TOKEN_SIZE,
            UserContent::Video(_) => IMAGE_MAX_TOKEN_SIZE,
            UserContent::Document(_) => IMAGE_MAX_TOKEN_SIZE,
        }
    }
}

impl TokenEstimator for OneOrMany<AssistantContent> {
    fn estimate_tokens(&self) -> u64 {
        self.iter().map(|content| content.estimate_tokens()).sum::<u64>()
    }
}

impl TokenEstimator for AssistantContent {
    fn estimate_tokens(&self) -> u64 {
        match self {
            AssistantContent::Text(t) => t.text.estimate_tokens(),
            AssistantContent::ToolCall(tc) => {
                let args = serde_json::to_string(&tc.function.arguments).unwrap_or_default();
                tc.function.name.estimate_tokens() + args.estimate_tokens()
            },
            AssistantContent::Reasoning(r) => {
                let mut sum = 0u64;
                for rc in &r.content {
                    sum += match rc {
                        ReasoningContent::Text { text, .. } => text.estimate_tokens(),
                        ReasoningContent::Encrypted(s) => s.estimate_tokens(),
                        ReasoningContent::Redacted { data } => data.estimate_tokens(),
                        ReasoningContent::Summary(s) => s.estimate_tokens(),
                        _ => 0,
                    };
                }
                sum
            },
            AssistantContent::Image(_) => IMAGE_MAX_TOKEN_SIZE,
        }
    }
}

impl TokenEstimator for OneOrMany<ToolResultContent> {
    fn estimate_tokens(&self) -> u64 {
        self.iter().map(|content| content.estimate_tokens()).sum::<u64>()
    }
}

impl TokenEstimator for ToolResultContent {
    fn estimate_tokens(&self) -> u64 {
        match self {
            ToolResultContent::Text(t) => t.text.estimate_tokens(),
            ToolResultContent::Image(_) => IMAGE_MAX_TOKEN_SIZE,
            ToolResultContent::Json { value } => value.to_string().estimate_tokens(),
        }
    }
}

const IMAGE_MAX_TOKEN_SIZE: u64 = 2_000;
const CHARS_PER_TOKEN: u64 = 4;
const CONSERVATIVE_NUM: u64 = 4;
const CONSERVATIVE_DEN: u64 = 3;
