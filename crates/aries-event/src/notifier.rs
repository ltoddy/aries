use rig::agent::MultiTurnStreamItem;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use crate::event::SessionMessageChunk;
use crate::{AgentEvent, Compaction, CompactionStatus, SubagentUpdate};

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
        let text = text.into();
        let event = AgentEvent::Notification(text);
        let _ = self.sender.send(event);
    }

    pub fn send_stream_item(&self, stream_item: MultiTurnStreamItem) {
        let event = AgentEvent::StreamItem(Box::new(stream_item));
        let _ = self.sender.send(event);
    }

    pub fn send_subagent_update(&self, subagent_update: SubagentUpdate) {
        let event = AgentEvent::SubagentUpdate(subagent_update);
        let _ = self.sender.send(event);
    }

    pub fn send_session_message_chunk(
        &self,
        message_id: impl Into<String>,
        content: MultiTurnStreamItem,
    ) {
        let event = AgentEvent::SessionMessageChunk(Box::new(SessionMessageChunk::new(
            message_id, content,
        )));
        let _ = self.sender.send(event);
    }

    pub fn send_compaction(
        &self,
        id: impl Into<String>,
        status: CompactionStatus,
        summary: impl Into<String>,
    ) {
        let id = id.into();
        let summary = summary.into();

        let event = AgentEvent::Compaction(Compaction::new(id, status, summary));
        let _ = self.sender.send(event);
    }

    pub fn send_session_info_update(
        &self,
        title: impl Into<String>,
        updated_at: impl Into<String>,
    ) {
        let title = title.into();
        let updated_at = updated_at.into();
        let event = AgentEvent::SessionInfoUpdate { title, updated_at };
        let _ = self.sender.send(event);
    }
}
