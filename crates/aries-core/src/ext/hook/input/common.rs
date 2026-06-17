use std::fmt::{Display, Formatter};

use serde::Serialize;

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
/// TODO: 能需要手动的为 Effort 实现序列化, 因为需要序列化成 {"level": "xxx"} 这种格式
pub enum Effort {
    Low,
    #[default]
    Medium,
    High,
    Xhigh,
    Max,
}

impl Display for Effort {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Effort::Low => write!(f, "low"),
            Effort::Medium => write!(f, "medium"),
            Effort::High => write!(f, "high"),
            Effort::Xhigh => write!(f, "xhigh"),
            Effort::Max => write!(f, "max"),
        }
    }
}

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PostCompactTrigger {
    #[default]
    Auto,
    Manual,
}

impl Display for PostCompactTrigger {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            PostCompactTrigger::Auto => write!(f, "auto"),
            PostCompactTrigger::Manual => write!(f, "manual"),
        }
    }
}
