use rig_core::agent::MultiTurnStreamItem;
use rig_core::message::Text;
use rig_core::streaming::StreamedAssistantContent;

use crate::tools::update_plan::PlanEntry;

#[derive(Debug, Clone)]
pub struct AgentEvent {
    pub main: bool,   // 是否是 main agent
    pub name: String, // agent name
    pub signal: AgentSignal,
}

impl AgentEvent {
    pub fn from_stream<R>(
        main: bool,
        name: impl Into<String>,
        item: MultiTurnStreamItem<R>,
    ) -> Self {
        let name = name.into();
        let item = earse(item);
        Self { main, name, signal: AgentSignal::Stream(item) }
    }

    pub fn from_plan(main: bool, name: impl Into<String>, entries: Vec<PlanEntry>) -> Self {
        Self { main, name: name.into(), signal: AgentSignal::PlanUpdate(entries) }
    }
}

#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum AgentSignal {
    Stream(MultiTurnStreamItem<()>),
    PlanUpdate(Vec<PlanEntry>),
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
        _ => MultiTurnStreamItem::StreamAssistantItem(StreamedAssistantContent::Text(Text::new(
            String::new(),
        ))), // unreachable
    }
}
