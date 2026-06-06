use std::collections::HashMap;

use agent_client_protocol::schema::{
    self, Content, ContentBlock, ContentChunk, Diff, SessionUpdate, TextContent, ToolCallContent,
    ToolCallId, ToolCallLocation, ToolCallStatus, ToolCallUpdate, ToolCallUpdateFields, ToolKind,
};
use aries_core::event::AgentEvent;
use aries_core::tools::{
    agent, bash, codesearch, edit, format_tool_args, format_tool_output, glob, grep, ls, multiedit,
    read, skill, webfetch, websearch, write,
};
use itertools::Itertools;
use parking_lot::Mutex;
use rig_core::agent::{MultiTurnStreamItem, Text};
use rig_core::message::{ReasoningContent, ToolCall, ToolFunction, ToolResultContent};
use rig_core::streaming::{StreamedAssistantContent, StreamedUserContent};

pub struct SessionUpdates(Vec<SessionUpdate>);

impl SessionUpdates {
    pub fn new(event: AgentEvent, tool_calls: &Mutex<HashMap<String, ToolCall>>) -> Self {
        match event.item {
            MultiTurnStreamItem::StreamAssistantItem(v) => {
                Self(Self::from_stream_assistant_content(v, tool_calls))
            },
            MultiTurnStreamItem::StreamUserItem(v) => {
                Self(Self::from_stream_user_content(v, tool_calls))
            },
            _ => Self(Vec::new()),
        }
    }

    fn from_stream_assistant_content(
        content: StreamedAssistantContent<()>,
        tool_calls: &Mutex<HashMap<String, ToolCall>>,
    ) -> Vec<SessionUpdate> {
        match content {
            StreamedAssistantContent::Text(Text { text }) => {
                vec![SessionUpdate::AgentMessageChunk(ContentChunk::new(ContentBlock::Text(
                    TextContent::new(text),
                )))]
            },
            StreamedAssistantContent::Reasoning(reasoning) => reasoning
                .content
                .into_iter()
                .filter_map(|rc| match rc {
                    ReasoningContent::Text { text, .. } => Some(text),
                    ReasoningContent::Encrypted(s) => Some(s),
                    ReasoningContent::Redacted { data } => Some(data),
                    ReasoningContent::Summary(s) => Some(s),
                    _ => None,
                })
                .map(|text| {
                    SessionUpdate::AgentThoughtChunk(ContentChunk::new(ContentBlock::Text(
                        TextContent::new(text),
                    )))
                })
                .collect(),
            StreamedAssistantContent::ReasoningDelta { reasoning, .. } => {
                vec![SessionUpdate::AgentThoughtChunk(ContentChunk::new(ContentBlock::Text(
                    TextContent::new(reasoning),
                )))]
            },
            StreamedAssistantContent::ToolCall { tool_call, internal_call_id, .. } => {
                tool_calls.lock().insert(internal_call_id, tool_call.clone());

                let (title, content) = title_and_content(tool_call.clone());
                let locations = locations(tool_call.clone()).unwrap_or_default();

                let ToolFunction { name, arguments } = tool_call.function;

                let acp_tool_call = schema::ToolCall::new(ToolCallId::new(tool_call.id), title)
                    .kind(tool_kind(&Some(name.clone())))
                    .status(ToolCallStatus::InProgress)
                    .content(content)
                    .locations(locations)
                    .raw_input(arguments);
                vec![SessionUpdate::ToolCall(acp_tool_call)]
            },
            _ => Vec::new(),
        }
    }

    fn from_stream_user_content(
        content: StreamedUserContent,
        tool_calls: &Mutex<HashMap<String, ToolCall>>,
    ) -> Vec<SessionUpdate> {
        match content {
            StreamedUserContent::ToolResult { tool_result, internal_call_id } => {
                let raw_output = tool_result
                    .content
                    .into_iter()
                    .filter_map(|c| match c {
                        ToolResultContent::Text(text) => Some(text.text),
                        _ => None,
                    })
                    .join("\n");

                let tool_call = tool_calls.lock().get(&internal_call_id).cloned();
                let (name, raw_input) = match tool_call {
                    Some(t) => (Some(t.function.name), Some(t.function.arguments)),
                    None => (None, None),
                };

                let content = if let Some(ref name) = name {
                    let output = format_tool_output(name, &raw_output);
                    Some(vec![ToolCallContent::from(ContentBlock::Text(TextContent::new(output)))])
                } else {
                    None
                };

                let fields = ToolCallUpdateFields::new()
                    .kind(tool_kind(&name))
                    .status(ToolCallStatus::Completed)
                    .content(content)
                    .raw_input(raw_input)
                    .raw_output(serde_json::Value::String(raw_output));

                let tool_call_update = ToolCallUpdate::new(ToolCallId::new(tool_result.id), fields);
                let tool_call_update = SessionUpdate::ToolCallUpdate(tool_call_update);
                vec![tool_call_update]
            },
        }
    }
}

impl IntoIterator for SessionUpdates {
    type Item = SessionUpdate;
    type IntoIter = std::vec::IntoIter<SessionUpdate>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

fn title_and_content(t: ToolCall) -> (String, Vec<ToolCallContent>) {
    let ToolFunction { name, arguments, .. } = t.function;

    let (args, _) = format_tool_args(&name, &arguments.to_string());

    let title = format!("{name}: {args}");
    let content = match name.as_str() {
        agent::NAME => serde_json::from_value::<agent::AgentArgs>(arguments)
            .map(|args| vec![ToolCallContent::Content(Content::new(args.prompt))])
            .unwrap_or_default(),
        edit::NAME => serde_json::from_value::<edit::EditArgs>(arguments)
            .map(|args| {
                vec![ToolCallContent::Diff(
                    Diff::new(args.file_path, args.new_text).old_text(args.old_text),
                )]
            })
            .unwrap_or_default(),
        multiedit::NAME => serde_json::from_value::<multiedit::MultiEditArgs>(arguments)
            .map(|args| {
                args.edits
                    .into_iter()
                    .map(|e| {
                        ToolCallContent::Diff(
                            Diff::new(args.file_path.clone(), e.new_text).old_text(e.old_text),
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default(),
        write::NAME => serde_json::from_value::<write::WriteArgs>(arguments)
            .map(|args| vec![ToolCallContent::Diff(Diff::new(args.file_path, args.content))])
            .unwrap_or_default(),
        _ => vec![],
    };

    (title, content)
}

fn tool_kind(tool_name: &Option<String>) -> ToolKind {
    match tool_name {
        Some(tool_name) => match tool_name.as_str() {
            glob::NAME | ls::NAME | read::NAME => ToolKind::Read,
            edit::NAME | multiedit::NAME | write::NAME => ToolKind::Edit,
            grep::NAME | codesearch::NAME => ToolKind::Search,
            bash::NAME => ToolKind::Execute,
            webfetch::NAME | websearch::NAME => ToolKind::Fetch,
            agent::NAME | skill::NAME => ToolKind::Think,
            _ => ToolKind::Other,
        },
        None => Default::default(),
    }
}

fn locations(t: ToolCall) -> Option<Vec<ToolCallLocation>> {
    let name: &str = &t.function.name;
    let arguments = t.function.arguments;

    match name {
        read::NAME => serde_json::from_value::<read::ReadArgs>(arguments)
            .map(|args| {
                let line = args.offset.and_then(|o| u32::try_from(o).ok());
                vec![ToolCallLocation::new(args.file_path).line(line)]
            })
            .ok(),
        write::NAME => serde_json::from_value::<write::WriteArgs>(arguments)
            .map(|args| vec![ToolCallLocation::new(args.file_path)])
            .ok(),
        edit::NAME => serde_json::from_value::<edit::EditArgs>(arguments)
            .map(|args| vec![ToolCallLocation::new(args.file_path)])
            .ok(),
        multiedit::NAME => serde_json::from_value::<multiedit::MultiEditArgs>(arguments)
            .map(|args| vec![ToolCallLocation::new(args.file_path)])
            .ok(),
        ls::NAME => serde_json::from_value::<ls::LsArgs>(arguments)
            .ok()
            .and_then(|args| args.path)
            .map(|path| vec![ToolCallLocation::new(path)]),
        glob::NAME => serde_json::from_value::<glob::GlobArgs>(arguments)
            .ok()
            .and_then(|args| args.base_dir)
            .map(|path| vec![ToolCallLocation::new(path)]),
        _ => None,
    }
}
