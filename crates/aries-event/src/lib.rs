use rig_agent::agent::{MultiTurnStreamItem, Text};
use rig_core::streaming::StreamedAssistantContent;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

#[derive(Debug, Clone)]
pub enum AgentEvent {
    Notification(String),
    StreamItem(MultiTurnStreamItem<()>),
}

impl AgentEvent {
    pub fn notification(text: impl Into<String>) -> Self {
        let text = text.into();
        Self::Notification(text)
    }

    pub fn stream_item<R>(stream_item: MultiTurnStreamItem<R>) -> Self {
        let stream_item = earse(stream_item);
        Self::StreamItem(stream_item)
    }
}

pub fn earse<R>(stream_item: MultiTurnStreamItem<R>) -> MultiTurnStreamItem<()> {
    match stream_item {
        MultiTurnStreamItem::StreamAssistantItem(content) => {
            match content {
                StreamedAssistantContent::Text(t) => {
                    MultiTurnStreamItem::StreamAssistantItem(StreamedAssistantContent::Text(t))
                },
                StreamedAssistantContent::ToolCall { tool_call, internal_call_id } => {
                    MultiTurnStreamItem::StreamAssistantItem(StreamedAssistantContent::ToolCall {
                        tool_call,
                        internal_call_id,
                    })
                },
                StreamedAssistantContent::ToolCallDelta { id, internal_call_id, content } => {
                    MultiTurnStreamItem::StreamAssistantItem(
                        StreamedAssistantContent::ToolCallDelta { id, internal_call_id, content },
                    )
                },
                StreamedAssistantContent::Reasoning(r) => {
                    MultiTurnStreamItem::StreamAssistantItem(StreamedAssistantContent::Reasoning(r))
                },
                StreamedAssistantContent::ReasoningDelta { id, reasoning } => {
                    MultiTurnStreamItem::StreamAssistantItem(
                        StreamedAssistantContent::ReasoningDelta { id, reasoning },
                    )
                },
                StreamedAssistantContent::Final(_) => {
                    MultiTurnStreamItem::StreamAssistantItem(StreamedAssistantContent::Final(()))
                },
                StreamedAssistantContent::Unknown(v) => {
                    MultiTurnStreamItem::StreamAssistantItem(StreamedAssistantContent::Unknown(v))
                },
            }
        },
        MultiTurnStreamItem::StreamUserItem(item) => MultiTurnStreamItem::StreamUserItem(item),
        MultiTurnStreamItem::FinalResponse(f) => MultiTurnStreamItem::FinalResponse(f),
        _ => MultiTurnStreamItem::StreamAssistantItem(StreamedAssistantContent::Text(Text::new(
            String::new(),
        ))), // unreachable
    }
}

// TODO 发送与接收消息未来统一使用此结构体
#[derive(Debug)]
pub struct Notifier<T: Clone> {
    sender: UnboundedSender<T>,
    receiver: UnboundedReceiver<T>,
}

impl<T: Clone> Notifier<T> {
    pub fn new() -> Self {
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel::<T>();

        Self { sender, receiver }
    }

    pub fn send(&self, event: T) {
        let _ = self.sender.send(event);
    }
}

impl<T: Clone> Default for Notifier<T> {
    fn default() -> Self {
        Self::new()
    }
}
