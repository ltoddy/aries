use agent_client_protocol::v1::schema::{
    AudioContent, BlobResourceContents, CompactionStatus, CompactionUpdate, Content, ContentBlock,
    ContentChunk, Diff, EmbeddedResource, EmbeddedResourceResource, ImageContent, MessageId, Plan,
    ResourceLink, SessionId, SessionInfoUpdate, SessionMessage, SessionMessageChunk, SessionUpdate,
    SubagentUpdate, Terminal, TextContent, TextResourceContents, ToolCall as AcpToolCall,
    ToolCallContent, ToolCallId, ToolCallLocation, ToolCallStatus, ToolCallUpdate,
    ToolCallUpdateFields, ToolKind, UsageUpdate,
};
use aries_communicate::AgentEvent;
use aries_runtime::tools::{
    agent, bash, batch, codesearch, edit, glob, grep, monitor, multiedit, read, skill, task_output,
    task_stop, update_plan, webfetch, websearch, write,
};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use rig::agent::{CompletionCall, MultiTurnStreamItem, PromptResponse};
use rig::completion::Message;
use rig::message::{
    AssistantContent, Audio, Document, DocumentData, DocumentSourceKind, Image, MimeType, ToolCall,
    ToolFunction, ToolResult, UserContent, Video,
};
use rig::streaming::{Item, StreamEvent};

use crate::v1::prompt::plan::PlanEntry;

#[derive(Clone, Debug, Default)]
pub struct SessionUpdates(Vec<SessionUpdate>);

impl SessionUpdates {
    pub fn new(event: AgentEvent) -> Self {
        match event {
            AgentEvent::Notification(text) => Self(vec![SessionUpdate::AgentMessageChunk(
                ContentChunk::new(ContentBlock::from(text)),
            )]),
            AgentEvent::StreamItem(stream_item) => match *stream_item {
                MultiTurnStreamItem::StreamAssistantItem(v) => {
                    Self(Self::from_stream_assistant_content(v))
                },
                MultiTurnStreamItem::ToolResult { tool_result } => {
                    Self(Self::from_tool_result(tool_result))
                },
                MultiTurnStreamItem::CompletionCall(call) => Self(Self::from_completion_call(call)),
                MultiTurnStreamItem::FinalResponse(res) => Self(Self::from_prompt_response(res)),
                MultiTurnStreamItem::ToolExecutionCommitted { tool_call } => {
                    Self(vec![SessionUpdate::ToolCallUpdate(ToolCallUpdate::new(
                        tool_call.id.to_string(),
                        ToolCallUpdateFields::new().status(ToolCallStatus::Completed),
                    ))]) // TODO 需要考虑工具调用失败的情况
                },
                MultiTurnStreamItem::ModelTurnRetried { turn: _ } => Default::default(),
                MultiTurnStreamItem::ToolCall { tool_call } => {
                    Self(vec![Self::from_tool_call(tool_call)])
                },
                _ => Default::default(),
            },
            AgentEvent::Compaction(compaction) => Self(vec![Self::from_compaction(compaction)]),
            AgentEvent::SessionInfoUpdate { title, updated_at } => {
                Self(vec![SessionUpdate::SessionInfoUpdate(
                    SessionInfoUpdate::new().title(title).updated_at(updated_at),
                )])
            },
            AgentEvent::SubagentUpdate(subagent_update) => {
                Self(vec![Self::from_subagent_update(subagent_update)])
            },
            AgentEvent::SessionMessage(message) => Self(vec![Self::from_session_message(message)]),
            AgentEvent::SessionMessageChunk(chunk) => {
                Self(vec![Self::from_session_message_chunk(*chunk)])
            },
        }
    }

    pub fn from_history(messages: Vec<Message>) -> Self {
        let updates = messages.into_iter().flat_map(Self::from_message).collect();
        Self(updates)
    }

    fn from_message(message: Message) -> Vec<SessionUpdate> {
        match message {
            Message::System { content: _ } => vec![],
            Message::User { content } => {
                content.into_iter().flat_map(Self::from_user_content).collect()
            },
            Message::Assistant(message) => {
                message.content.into_iter().flat_map(Self::from_assistant_content).collect()
            },
        }
    }

    fn from_user_content(content: UserContent) -> Vec<SessionUpdate> {
        let block = match content {
            UserContent::Text(text) => ContentBlock::from(text.text),
            UserContent::ToolResult(tool_result) => return Self::from_tool_result(tool_result),
            UserContent::Image(image) => Self::from_image(image),
            UserContent::Audio(audio) => Self::from_audio(audio),
            UserContent::Video(video) => Self::from_video(video),
            UserContent::Document(document) => Self::from_document(document),
        };
        vec![SessionUpdate::UserMessageChunk(ContentChunk::new(block))]
    }

    fn from_assistant_content(content: AssistantContent) -> Vec<SessionUpdate> {
        match content {
            AssistantContent::Text(text) => vec![SessionUpdate::AgentMessageChunk(
                ContentChunk::new(ContentBlock::from(text.text)),
            )],
            AssistantContent::ToolCall(tool_call) => vec![Self::from_tool_call(tool_call)],
            AssistantContent::Reasoning(reasoning) => vec![SessionUpdate::AgentThoughtChunk(
                ContentChunk::new(ContentBlock::from(reasoning.text)),
            )],
            AssistantContent::Image(_) => vec![SessionUpdate::AgentMessageChunk(
                ContentChunk::new(ContentBlock::from("[image]")),
            )],
            AssistantContent::Opaque(_) => vec![],
            _ => vec![],
        }
    }

    fn from_stream_assistant_content(content: Item<StreamEvent>) -> Vec<SessionUpdate> {
        match content {
            Item::Event(event) => match event {
                StreamEvent::Start { part: _, kind: _, name: _ } => {},
                StreamEvent::Text { part: _, text } => {
                    return vec![SessionUpdate::AgentMessageChunk(ContentChunk::new(
                        ContentBlock::from(text),
                    ))];
                },
                StreamEvent::Reasoning { part: _, text } => {
                    return vec![SessionUpdate::AgentThoughtChunk(ContentChunk::new(
                        ContentBlock::from(text),
                    ))];
                },
                StreamEvent::Arguments { part: _, json: _ } => {},
                StreamEvent::End { part: _, content } => match content {
                    AssistantContent::Text(_text) => {},
                    AssistantContent::ToolCall(tool_call) => {
                        return vec![Self::from_tool_call(tool_call)];
                    },
                    AssistantContent::Reasoning(_reasoning) => {},
                    AssistantContent::Image(_image) => {},
                    AssistantContent::Opaque(_) => {},
                    _ => {},
                },
            },
            Item::Unknown(_payload) => {},
        }

        vec![]
    }

    fn from_completion_call(call: CompletionCall) -> Vec<SessionUpdate> {
        let total_tokens = call.usage.total_tokens.unwrap_or_default();
        vec![SessionUpdate::UsageUpdate(UsageUpdate::new(total_tokens, 0))]
    }

    fn from_tool_result(tool_result: ToolResult) -> Vec<SessionUpdate> {
        let raw_output = tool_result.content.first().and_then(|content| content.as_json()).cloned();

        if tool_result.name.as_str() == update_plan::NAME
            && let Some(output) = raw_output.clone()
            && let Ok(entries) = serde_json::from_value::<Vec<update_plan::PlanEntry>>(output)
        {
            return vec![Self::from_plan_entries(entries)];
        }

        let fields =
            ToolCallUpdateFields::new().status(ToolCallStatus::Completed).raw_output(raw_output);

        vec![SessionUpdate::ToolCallUpdate(ToolCallUpdate::new(
            ToolCallId::new(tool_result.call.to_string()),
            fields,
        ))]
    }

    fn from_image(image: Image) -> ContentBlock {
        let mime_type = image.media_type.as_ref().map(MimeType::to_mime_type);
        match (image.data, mime_type) {
            (DocumentSourceKind::Base64(data), Some(mime_type)) => {
                ContentBlock::Image(ImageContent::new(data, mime_type))
            },
            (DocumentSourceKind::Raw(bytes), Some(mime_type)) => {
                ContentBlock::Image(ImageContent::new(STANDARD.encode(bytes), mime_type))
            },
            (DocumentSourceKind::Url(uri), mime_type) => ContentBlock::ResourceLink(
                ResourceLink::new("image", uri).mime_type(mime_type.map(str::to_owned)),
            ),
            (DocumentSourceKind::FileId(id), _) => {
                ContentBlock::from(format!("[image: provider file ID {id}]"))
            },
            (DocumentSourceKind::Unknown, _) => ContentBlock::from("[image: source unavailable]"),
            (DocumentSourceKind::Base64(data), None) => ContentBlock::Resource(
                EmbeddedResource::new(EmbeddedResourceResource::BlobResourceContents(
                    BlobResourceContents::new(data, "urn:aries:image"),
                )),
            ),
            (DocumentSourceKind::Raw(bytes), None) => ContentBlock::Resource(
                EmbeddedResource::new(EmbeddedResourceResource::BlobResourceContents(
                    BlobResourceContents::new(STANDARD.encode(bytes), "urn:aries:image"),
                )),
            ),
        }
    }

    fn from_audio(audio: Audio) -> ContentBlock {
        let mime_type = audio.media_type.as_ref().map(MimeType::to_mime_type);
        match (audio.data, mime_type) {
            (DocumentSourceKind::Base64(data), Some(mime_type)) => {
                ContentBlock::Audio(AudioContent::new(data, mime_type))
            },
            (DocumentSourceKind::Raw(bytes), Some(mime_type)) => {
                ContentBlock::Audio(AudioContent::new(STANDARD.encode(bytes), mime_type))
            },
            (DocumentSourceKind::Url(uri), mime_type) => ContentBlock::ResourceLink(
                ResourceLink::new("audio", uri).mime_type(mime_type.map(str::to_owned)),
            ),
            (DocumentSourceKind::FileId(id), _) => {
                ContentBlock::from(format!("[audio: provider file ID {id}]"))
            },
            (DocumentSourceKind::Unknown, _) => ContentBlock::from("[audio: source unavailable]"),
            (DocumentSourceKind::Base64(data), None) => ContentBlock::Resource(
                EmbeddedResource::new(EmbeddedResourceResource::BlobResourceContents(
                    BlobResourceContents::new(data, "urn:aries:audio"),
                )),
            ),
            (DocumentSourceKind::Raw(bytes), None) => ContentBlock::Resource(
                EmbeddedResource::new(EmbeddedResourceResource::BlobResourceContents(
                    BlobResourceContents::new(STANDARD.encode(bytes), "urn:aries:audio"),
                )),
            ),
        }
    }

    fn from_video(_video: Video) -> ContentBlock {
        ContentBlock::from("[video]")
    }

    fn from_document(document: Document) -> ContentBlock {
        let mime_type = document.media_type.as_ref().map(MimeType::to_mime_type);
        let source = match document.data {
            DocumentData::Text(text) => {
                return ContentBlock::Resource(EmbeddedResource::new(
                    EmbeddedResourceResource::TextResourceContents(
                        TextResourceContents::new(text, "urn:aries:document")
                            .mime_type(mime_type.map(str::to_owned)),
                    ),
                ));
            },
            DocumentData::File(source) => source,
        };
        let data = match source {
            DocumentSourceKind::Base64(data) => data,
            DocumentSourceKind::Raw(bytes) => STANDARD.encode(bytes),
            DocumentSourceKind::Url(uri) => {
                return ContentBlock::ResourceLink(
                    ResourceLink::new("document", uri).mime_type(mime_type.map(str::to_owned)),
                );
            },
            DocumentSourceKind::FileId(id) => {
                return ContentBlock::from(format!("[document: provider file ID {id}]"));
            },
            DocumentSourceKind::Unknown => {
                return ContentBlock::from("[document: source unavailable]");
            },
        };
        ContentBlock::Resource(EmbeddedResource::new(
            EmbeddedResourceResource::BlobResourceContents(
                BlobResourceContents::new(data, "urn:aries:document")
                    .mime_type(mime_type.map(str::to_owned)),
            ),
        ))
    }

    fn from_plan_entries(entries: Vec<update_plan::PlanEntry>) -> SessionUpdate {
        let entries = entries.into_iter().map(|e| PlanEntry::new(e).into()).collect();
        let plan = Plan::new(entries);
        SessionUpdate::Plan(plan)
    }

    fn from_prompt_response(res: PromptResponse) -> Vec<SessionUpdate> {
        let mut updates = vec![];
        if let Some(completion) = res.completion_calls.last() {
            let total_tokens = completion.usage.total_tokens.unwrap_or_default();
            updates.push(SessionUpdate::UsageUpdate(UsageUpdate::new(total_tokens, 0)));
        }
        updates
    }

    fn from_tool_call(tool_call: ToolCall) -> SessionUpdate {
        let (title, content) = parse_tool_call(&tool_call.function);
        let locations = locations(&tool_call.function);
        let ToolFunction { name, arguments, invalid_arguments: _ } = tool_call.function;
        let arguments = serde_json::Value::Object(arguments);
        let kind = tool_kind(name.as_str());

        let tc = AcpToolCall::new(ToolCallId::new(tool_call.id.to_string()), title)
            .kind(kind)
            .status(ToolCallStatus::InProgress)
            .content(content)
            .locations(locations)
            .raw_input(arguments);

        SessionUpdate::ToolCall(tc)
    }

    fn from_compaction(compaction: aries_communicate::Compaction) -> SessionUpdate {
        let status = match compaction.status {
            aries_communicate::CompactionStatus::InProgress => CompactionStatus::InProgress,
            aries_communicate::CompactionStatus::Completed => CompactionStatus::Completed,
            aries_communicate::CompactionStatus::Failed => CompactionStatus::Failed,
            aries_communicate::CompactionStatus::Cancelled => CompactionStatus::Cancelled,
        };

        SessionUpdate::CompactionUpdate(
            CompactionUpdate::new(compaction.id, status)
                .summary(vec![ContentBlock::from(compaction.summary)]),
        )
    }

    fn from_subagent_update(subagent_update: aries_communicate::SubagentUpdate) -> SessionUpdate {
        SessionUpdate::SubagentUpdate(
            SubagentUpdate::new(subagent_update.session_id)
                .title(subagent_update.title)
                .description(subagent_update.description),
        )
    }

    fn from_session_message(message: aries_communicate::SessionMessage) -> SessionUpdate {
        SessionUpdate::SessionMessage(
            SessionMessage::new(message.message_id)
                .sender_session_id(SessionId::new(message.sender_session_id))
                .recipient_session_id(SessionId::new(message.recipient_session_id))
                .sender_message_id(MessageId::new(message.sender_message_id))
                .recipient_message_id(MessageId::new(message.recipient_message_id)),
        )
    }

    fn from_session_message_chunk(chunk: aries_communicate::SessionMessageChunk) -> SessionUpdate {
        let content: ContentBlock = match chunk.content {
            MultiTurnStreamItem::StreamAssistantItem(item) => match item {
                Item::Event(event) => match event {
                    StreamEvent::Text { part: _, text } => {
                        ContentBlock::Text(TextContent::new(text))
                    },
                    StreamEvent::Reasoning { part: _, text } => {
                        ContentBlock::Text(TextContent::new(text))
                    },
                    StreamEvent::End { part: _, content: _ } => {
                        ContentBlock::Text(TextContent::new("\n"))
                    },
                    _ => ContentBlock::Text(TextContent::new("")),
                },
                Item::Unknown(payload) => {
                    ContentBlock::Text(TextContent::new(payload.value().to_string()))
                },
            },
            MultiTurnStreamItem::ToolCall { tool_call } => {
                let (title, _) = parse_tool_call(&tool_call.function);
                ContentBlock::Text(TextContent::new(format!("\n{title}\n")))
            },
            // MultiTurnStreamItem::ToolExecutionCommitted { .. } => {},
            // MultiTurnStreamItem::ToolResult { .. } => {},
            // MultiTurnStreamItem::CompletionCall(_) => {},
            // MultiTurnStreamItem::ModelTurnRetried { .. } => {},
            // MultiTurnStreamItem::FinalResponse(_) => {},
            _ => ContentBlock::Text(TextContent::new("")),
        };

        SessionUpdate::SessionMessageChunk(SessionMessageChunk::new(chunk.message_id, content))
    }
}

impl IntoIterator for SessionUpdates {
    type Item = SessionUpdate;
    type IntoIter = std::vec::IntoIter<SessionUpdate>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

fn parse_tool_call(function: &ToolFunction) -> (String, Vec<ToolCallContent>) {
    let ToolFunction { name, arguments, invalid_arguments: _ } = function;
    let arguments = serde_json::Value::Object(arguments.clone());

    let default_title = format!("{name}: {arguments}");
    match name.as_str() {
        agent::NAME => serde_json::from_value::<agent::AgentArgs>(arguments)
            .map(|args| (args.title(), vec![ToolCallContent::Content(Content::new(args.prompt))]))
            .unwrap_or_else(|_| (default_title, vec![])),
        bash::NAME => serde_json::from_value::<bash::BashArgs>(arguments)
            .map(|args| (args.title(), vec![]))
            .unwrap_or_else(|_| {
                (
                    default_title,
                    vec![ToolCallContent::Terminal(Terminal::new(
                        std::env::var("SHELL").unwrap_or_else(|_| "bash".to_owned()),
                    ))],
                )
            }),
        batch::NAME => serde_json::from_value::<batch::BatchArgs>(arguments)
            .map(|args| (args.title(), vec![]))
            .unwrap_or_else(|_| (default_title, vec![])),
        glob::NAME => serde_json::from_value::<glob::GlobArgs>(arguments)
            .map(|args| (args.title(), vec![]))
            .unwrap_or_else(|_| (default_title, vec![])),
        grep::NAME => serde_json::from_value::<grep::GrepArgs>(arguments)
            .map(|args| (args.title(), vec![]))
            .unwrap_or_else(|_| (default_title, vec![])),
        monitor::NAME => serde_json::from_value::<monitor::MonitorArgs>(arguments)
            .map(|args| (args.title(), vec![]))
            .unwrap_or_else(|_| (default_title, vec![])),
        read::NAME => serde_json::from_value::<read::ReadArgs>(arguments)
            .map(|args| (args.title(), vec![]))
            .unwrap_or_else(|_| (default_title, vec![])),
        edit::NAME => serde_json::from_value::<edit::EditArgs>(arguments)
            .map(|args| {
                (
                    args.title(),
                    vec![ToolCallContent::Diff(
                        Diff::new(args.file_path, args.new_text).old_text(args.old_text),
                    )],
                )
            })
            .unwrap_or_else(|_| (default_title, vec![])),
        multiedit::NAME => serde_json::from_value::<multiedit::MultiEditArgs>(arguments)
            .map(|args| {
                let title = args.title();
                let content = args
                    .edits
                    .into_iter()
                    .map(|e| {
                        ToolCallContent::Diff(
                            Diff::new(args.file_path.clone(), e.new_text).old_text(e.old_text),
                        )
                    })
                    .collect::<Vec<_>>();
                (title, content)
            })
            .unwrap_or_else(|_| (default_title, vec![])),
        write::NAME => serde_json::from_value::<write::WriteArgs>(arguments)
            .map(|args| {
                (args.title(), vec![ToolCallContent::Diff(Diff::new(args.file_path, args.content))])
            })
            .unwrap_or_else(|_| (default_title, vec![])),
        webfetch::NAME => serde_json::from_value::<webfetch::WebFetchArgs>(arguments)
            .map(|args| (args.title(), vec![]))
            .unwrap_or_else(|_| (default_title, vec![])),
        websearch::NAME => serde_json::from_value::<websearch::WebSearchArgs>(arguments)
            .map(|args| (args.title(), vec![]))
            .unwrap_or_else(|_| (default_title, vec![])),
        codesearch::NAME => serde_json::from_value::<codesearch::CodeSearchArgs>(arguments)
            .map(|args| (args.title(), vec![]))
            .unwrap_or_else(|_| (default_title, vec![])),
        skill::NAME => serde_json::from_value::<skill::SkillArgs>(arguments)
            .map(|args| (args.title(), vec![]))
            .unwrap_or_else(|_| (default_title, vec![])),
        task_output::NAME => serde_json::from_value::<task_output::TaskOutputArgs>(arguments)
            .map(|args| (args.title(), vec![]))
            .unwrap_or_else(|_| (default_title, vec![])),
        task_stop::NAME => serde_json::from_value::<task_stop::TaskStopArgs>(arguments)
            .map(|args| (args.title(), vec![]))
            .unwrap_or_else(|_| (default_title, vec![])),
        update_plan::NAME => serde_json::from_value::<update_plan::UpdatePlanArgs>(arguments)
            .map(|args| (args.title(), vec![]))
            .unwrap_or_else(|_| (default_title, vec![])),
        _ => (default_title, vec![]),
    }
}

fn tool_kind(tool_name: &str) -> ToolKind {
    match tool_name {
        glob::NAME | read::NAME | task_output::NAME => ToolKind::Read,
        edit::NAME | multiedit::NAME | write::NAME => ToolKind::Edit,
        grep::NAME | codesearch::NAME => ToolKind::Search,
        bash::NAME | batch::NAME | monitor::NAME | task_stop::NAME => ToolKind::Execute,
        webfetch::NAME | websearch::NAME => ToolKind::Fetch,
        agent::NAME | skill::NAME | update_plan::NAME => ToolKind::Think,
        _ => ToolKind::Other,
    }
}

fn locations(t: &ToolFunction) -> Vec<ToolCallLocation> {
    let arguments = serde_json::Value::Object(t.arguments.clone());

    match t.name.as_str() {
        read::NAME => {
            if let Ok(args) = serde_json::from_value::<read::ReadArgs>(arguments) {
                return vec![ToolCallLocation::new(args.location())];
            }
        },
        write::NAME => {
            if let Ok(args) = serde_json::from_value::<write::WriteArgs>(arguments) {
                return vec![ToolCallLocation::new(args.location())];
            }
        },
        edit::NAME => {
            if let Ok(args) = serde_json::from_value::<edit::EditArgs>(arguments) {
                return vec![ToolCallLocation::new(args.location())];
            }
        },
        multiedit::NAME => {
            if let Ok(args) = serde_json::from_value::<multiedit::MultiEditArgs>(arguments) {
                return vec![ToolCallLocation::new(args.location())];
            }
        },
        glob::NAME => {
            if let Ok(args) = serde_json::from_value::<glob::GlobArgs>(arguments)
                && let Some(base_dir) = args.base_dir
            {
                return vec![ToolCallLocation::new(base_dir)];
            }
        },
        _ => {},
    }

    vec![]
}
