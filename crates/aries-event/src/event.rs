use std::fmt;

use rig::agent::MultiTurnStreamItem;

#[derive(Debug, Clone)]
pub enum AgentEvent {
    Notification(String),
    StreamItem(Box<MultiTurnStreamItem>),
    SubagentUpdate(SubagentUpdate),
    SessionMessageChunk(Box<SessionMessageChunk>),
    Compaction(Compaction),
    SessionInfoUpdate { title: String, updated_at: String },
}

#[derive(Debug, Clone)]
pub struct SubagentUpdate {
    pub session_id: String,
    pub title: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SessionMessageChunk {
    pub message_id: String,
    pub sender_session_id: String,
    pub recipient_session_id: String,
    pub content: MultiTurnStreamItem,
}

impl SessionMessageChunk {
    pub fn new(
        message_id: impl Into<String>,
        sender_session_id: impl Into<String>,
        recipient_session_id: impl Into<String>,
        content: MultiTurnStreamItem,
    ) -> Self {
        let message_id = message_id.into();
        let sender_session_id = sender_session_id.into();
        let recipient_session_id = recipient_session_id.into();

        Self { message_id, sender_session_id, recipient_session_id, content }
    }
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

impl Compaction {
    pub fn new(
        id: impl Into<String>,
        status: CompactionStatus,
        summary: impl Into<String>,
    ) -> Self {
        let id = id.into();
        let summary = summary.into();

        Self { id, status, summary }
    }
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
