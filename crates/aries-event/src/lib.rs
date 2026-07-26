use rig_core::agent::MultiTurnStreamItem;
use rig_core::message::Text;
use rig_core::streaming::StreamedAssistantContent;

#[derive(Debug, Clone)]
pub struct AgentEvent {
    pub main: bool,   // 是否是 main agent
    pub name: String, // agent name
    pub stream_item: MultiTurnStreamItem<()>,
}

impl AgentEvent {
    pub fn text(main: bool, name: impl Into<String>, text: impl Into<String>) -> Self {
        let name = name.into();
        let text = text.into();
        let stream_item =
            MultiTurnStreamItem::StreamAssistantItem(StreamedAssistantContent::text(&text));
        // TODO: 这里使用 assistant 类型不是非常合适,但是又没有提供 user 类型

        Self { main, name, stream_item }
    }

    pub fn from_stream<R>(
        main: bool,
        name: impl Into<String>,
        stream_item: MultiTurnStreamItem<R>,
    ) -> Self {
        let name = name.into();
        let stream_item = earse(stream_item);
        Self { main, name, stream_item }
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
