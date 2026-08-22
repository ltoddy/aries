use rig::message::{
    AssistantContent, Document, DocumentSourceKind, Message, ReasoningContent, ToolResultContent,
    UserContent,
};

pub trait TokenEstimator {
    fn estimate_tokens(&self) -> u64;
}

impl TokenEstimator for &str {
    fn estimate_tokens(&self) -> u64 {
        if self.is_empty() {
            return 0;
        }

        let milli_tokens = self.chars().map(char_token_millis).sum::<u64>();
        milli_tokens.div_ceil(1_000).max(1)
    }
}

impl TokenEstimator for String {
    fn estimate_tokens(&self) -> u64 {
        if self.is_empty() {
            return 0;
        }

        let milli_tokens = self.chars().map(char_token_millis).sum::<u64>();
        milli_tokens.div_ceil(1_000).max(1)
    }
}

fn char_token_millis(c: char) -> u64 {
    if c.is_ascii() {
        ASCII_TOKEN_MILLIS_PER_CHAR
    } else if is_cjk(c) {
        CJK_TOKEN_MILLIS_PER_CHAR
    } else {
        OTHER_TOKEN_MILLIS_PER_CHAR
    }
}

fn is_cjk(c: char) -> bool {
    matches!(
        u32::from(c),
        0x2E80..=0x2FFF
            | 0x3000..=0x303F
            | 0x3040..=0x30FF
            | 0x3100..=0x312F
            | 0x3190..=0x319F
            | 0x3400..=0x4DBF
            | 0x4E00..=0x9FFF
            | 0xAC00..=0xD7AF
            | 0xF900..=0xFAFF
            | 0xFE30..=0xFE4F
            | 0xFF00..=0xFFEF
            | 0x20000..=0x3134F
    )
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

impl TokenEstimator for Vec<Message> {
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

impl TokenEstimator for Vec<UserContent> {
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
            UserContent::Audio(_) => MEDIA_MAX_TOKEN_SIZE,
            UserContent::Video(_) => MEDIA_MAX_TOKEN_SIZE,
            UserContent::Document(document) => document.estimate_tokens(),
        }
    }
}

impl TokenEstimator for Document {
    fn estimate_tokens(&self) -> u64 {
        self.data.estimate_tokens()
    }
}

impl TokenEstimator for DocumentSourceKind {
    fn estimate_tokens(&self) -> u64 {
        match self {
            DocumentSourceKind::Url(source)
            | DocumentSourceKind::Base64(source)
            | DocumentSourceKind::FileId(source)
            | DocumentSourceKind::String(source) => source.estimate_tokens(),
            DocumentSourceKind::Raw(bytes) => bytes.len().div_ceil(4) as u64,
            DocumentSourceKind::Unknown => 0,
        }
    }
}

impl TokenEstimator for Vec<AssistantContent> {
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
                    };
                }
                sum
            },
            AssistantContent::Image(_) => IMAGE_MAX_TOKEN_SIZE,
        }
    }
}

impl TokenEstimator for Vec<ToolResultContent> {
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
const MEDIA_MAX_TOKEN_SIZE: u64 = 20_000;

const ASCII_TOKEN_MILLIS_PER_CHAR: u64 = 250;
const CJK_TOKEN_MILLIS_PER_CHAR: u64 = 1_200;
const OTHER_TOKEN_MILLIS_PER_CHAR: u64 = 500;

const CONSERVATIVE_NUM: u64 = 4;
const CONSERVATIVE_DEN: u64 = 3;
