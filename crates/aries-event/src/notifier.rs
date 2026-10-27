use rig::agent::MultiTurnStreamItem;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use crate::{AgentEvent, CompactionStatus, SubagentUpdate};

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

    pub fn send_subagent_update(&self, subagent_update: SubagentUpdate) {
        let event = AgentEvent::subagent_update(subagent_update);
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
