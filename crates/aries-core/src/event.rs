use rig_core::agent::MultiTurnStreamItem;
use rig_core::message::Text;
use rig_core::streaming::StreamedAssistantContent;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct AgentEvent {
    pub name: String, // agent name
    pub item: MultiTurnStreamItem<()>,
}

impl AgentEvent {
    pub fn new<R>(name: impl Into<String>, item: MultiTurnStreamItem<R>) -> Self {
        let name = name.into();
        let item = earse(item);
        Self { name, item }
    }
}

pub fn earse<R>(item: MultiTurnStreamItem<R>) -> MultiTurnStreamItem<()> {
    match item {
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
            }
        },
        MultiTurnStreamItem::StreamUserItem(item) => MultiTurnStreamItem::StreamUserItem(item),
        MultiTurnStreamItem::FinalResponse(f) => MultiTurnStreamItem::FinalResponse(f),
        _ => MultiTurnStreamItem::StreamAssistantItem(StreamedAssistantContent::Text(Text {
            // unreachable
            text: String::new(),
        })),
    }
}
