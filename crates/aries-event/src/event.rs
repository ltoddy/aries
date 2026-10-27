use std::fmt;

use rig::agent::MultiTurnStreamItem;

#[derive(Debug, Clone)]
pub enum AgentEvent {
    Notification(String),
    StreamItem(Box<MultiTurnStreamItem>),
    SubagentUpdate(SubagentUpdate),
    Compaction(Compaction),
    SessionInfoUpdate { title: String, updated_at: String },
}

#[derive(Debug, Clone)]
pub struct SubagentUpdate {
    pub session_id: String,
    pub title: Option<String>,
    pub description: Option<String>,
}

impl SubagentUpdate {
    pub fn new(session_id: impl Into<String>) -> Self {
        let session_id = session_id.into();

        Self { session_id, title: None, description: None }
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        let title = title.into();
        self.title = Some(title);
        self
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        let description = description.into();
        self.description = Some(description);
        self
    }
}

#[derive(Debug, Clone)]
pub struct Compaction {
    pub id: String,
    pub status: CompactionStatus,
    pub summary: String,
}

#[derive(Debug, Default, Clone, Copy)]
pub enum CompactionStatus {
    #[default]
    InProgress,
    Completed,
    Failed,
    Cancelled,
}

impl fmt::Display for Compaction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Compaction {}: {}", self.status, self.summary)
    }
}

impl fmt::Display for CompactionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InProgress => f.write_str("in progress"),
            Self::Completed => f.write_str("completed"),
            Self::Failed => f.write_str("failed"),
            Self::Cancelled => f.write_str("cancelled"),
        }
    }
}

impl AgentEvent {
    pub fn notification(text: impl Into<String>) -> Self {
        let text = text.into();
        Self::Notification(text)
    }

    pub fn stream_item(stream_item: MultiTurnStreamItem) -> Self {
        Self::StreamItem(Box::new(stream_item))
    }

    pub fn subagent_update(subagent_update: SubagentUpdate) -> Self {
        Self::SubagentUpdate(subagent_update)
    }

    pub fn compaction(
        id: impl Into<String>,
        status: CompactionStatus,
        summary: impl Into<String>,
    ) -> Self {
        let id = id.into();
        let summary = summary.into();
        let compaction = Compaction { id, status, summary };
        Self::Compaction(compaction)
    }

    pub fn session_info_update(title: impl Into<String>, updated_at: impl Into<String>) -> Self {
        let title = title.into();
        let updated_at = updated_at.into();
        Self::SessionInfoUpdate { title, updated_at }
    }
}
