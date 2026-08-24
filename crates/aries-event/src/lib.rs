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

    pub fn send_awaiting_input(&self, args: serde_json::Value) {
        let event = AgentEvent::awaiting_user_input(args);
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
    AwaitingUserInput { args: serde_json::Value },
    SessionInfoUpdate { title: String, updated_at: String },
}

impl AgentEvent {
    pub fn notification(text: impl Into<String>) -> Self {
        let text = text.into();
        Self::Notification(text)
    }

    pub fn stream_item(stream_item: MultiTurnStreamItem) -> Self {
        Self::StreamItem(Box::new(stream_item))
    }

    pub fn awaiting_user_input(args: serde_json::Value) -> Self {
        Self::AwaitingUserInput { args }
    }

    pub fn session_info_update(title: impl Into<String>, updated_at: impl Into<String>) -> Self {
        let title = title.into();
        let updated_at = updated_at.into();
        Self::SessionInfoUpdate { title, updated_at }
    }
}
