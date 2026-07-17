use std::fmt::{Display, Formatter};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PlanEntry {
    pub content: String,
    pub priority: PlanEntryPriority,
    pub status: PlanEntryStatus,
}

impl Display for PlanEntry {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mark = self.status.mark();

        write!(f, "\t{mark}{}{}", self.priority, self.content)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanEntryPriority {
    High,
    Medium,
    Low,
}

impl Display for PlanEntryPriority {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            PlanEntryPriority::High => write!(f, "high"),
            PlanEntryPriority::Medium => write!(f, "medium"),
            PlanEntryPriority::Low => write!(f, "low"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanEntryStatus {
    Pending,
    InProgress,
    Completed,
}

impl PlanEntryStatus {
    pub fn mark(&self) -> &str {
        match self {
            PlanEntryStatus::Pending => "☐",
            PlanEntryStatus::InProgress => "◐",
            PlanEntryStatus::Completed => "☑",
        }
    }
}

impl Display for PlanEntryStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            PlanEntryStatus::Pending => write!(f, "pending"),
            PlanEntryStatus::InProgress => write!(f, "in-progress"),
            PlanEntryStatus::Completed => write!(f, "completed"),
        }
    }
}
