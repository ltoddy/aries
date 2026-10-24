use std::fmt;

use rig::agent::MultiTurnStreamItem;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

#[derive(Debug, Clone)]
pub struct Notifier {
    sender: UnboundedSender<AgentEvent>,
}

impl Notifier {
    pub fn channel() -> (Self, UnboundedReceiver<AgentEvent>) {
        let (sender, receiver) = unbounded_channel::<AgentEvent>();

        (Self { sender }, receiver)
    }

    //  fire and forget

    pub fn notify(&self, text: impl Into<String>) {
        let event = AgentEvent::notification(text);
        let _ = self.sender.send(event);
    }

    pub fn send_stream_item(&self, stream_item: MultiTurnStreamItem) {
        let event = AgentEvent::stream_item(stream_item);
        let _ = self.sender.send(event);
    }

    pub fn send_compaction(
        &self,
        id: impl Into<String>,
        status: CompactionStatus,
        summary: impl Into<String>,
    ) {
        let event = AgentEvent::compaction(id, status, summary);
        let _ = self.sender.send(event);
    }

    pub fn send_session_info_update(
        &self,
        title: impl Into<String>,
        updated_at: impl Into<String>,
    ) {
        let event = AgentEvent::session_info_update(title, updated_at);
        let _ = self.sender.send(event);
    }
}

#[derive(Debug, Clone)]
pub enum AgentEvent {
    Notification(String),
    StreamItem(Box<MultiTurnStreamItem>),
    Compaction(Compaction),
    SessionInfoUpdate { title: String, updated_at: String },
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
