use std::collections::HashMap;

use agent_client_protocol::schema::v1::{
    Content, ContentBlock, ContentChunk, Diff, Plan, SessionUpdate, ToolCall as AcpToolCall,
    ToolCallContent, ToolCallId, ToolCallLocation, ToolCallStatus, ToolCallUpdate,
    ToolCallUpdateFields, ToolKind, UsageUpdate,
};
use aries_event::AgentEvent;
use aries_tools::tools::format_tool_output;
use aries_tools::{
    agent, bash, batch, codesearch, edit, glob, grep, lsp, multiedit, question, read, skill,
    update_plan, webfetch, websearch, write,
};
use itertools::Itertools;
use parking_lot::Mutex;
use rig_agent::agent::MultiTurnStreamItem;
use rig_core::message::{ReasoningContent, ToolCall, ToolFunction, ToolResultContent};
use rig_core::streaming::{StreamedAssistantContent, StreamedUserContent};

use super::plan::PlanEntry;

pub struct SessionUpdates(Vec<SessionUpdate>);

impl SessionUpdates {
    pub fn new(event: AgentEvent, tool_calls: &Mutex<HashMap<String, ToolCall>>) -> Self {
        match event {
            AgentEvent::Notification(text) => Self(vec![SessionUpdate::AgentMessageChunk(
                ContentChunk::new(ContentBlock::from(text)),
            )]),
            AgentEvent::StreamItem(stream_item) => {
                match stream_item {
                    MultiTurnStreamItem::StreamAssistantItem(v) => {
                        Self(Self::from_stream_assistant_content(v, tool_calls))
                    },
                    MultiTurnStreamItem::StreamUserItem(v) => {
                        Self(Self::from_stream_user_content(v, tool_calls))
                    },
                    MultiTurnStreamItem::CompletionCall(_) => {
                        // TODO
                        Self(Vec::new())
                    },
                    MultiTurnStreamItem::FinalResponse(res) => {
                        let usage = res.usage();
                        let text = format!(
                            "\n\nThis turn token usage: input tokens = {} (cached = {}), output tokens = {}, total tokens = {}, reasoning tokens = {}",
                            usage.input_tokens,
                            usage.cached_input_tokens,
                            usage.output_tokens,
                            usage.total_tokens,
                            usage.reasoning_tokens,
                        );
                        if let Some(completion) = res.completion_calls.last() {
                            return Self(vec![
                                SessionUpdate::AgentMessageChunk(ContentChunk::new(
                                    ContentBlock::from(text),
                                )),
                                SessionUpdate::UsageUpdate(UsageUpdate::new(
                                    completion.usage.total_tokens,
                                    0,
                                )),
                            ]);
                        }
                        Self(Vec::new())
                    },
                    MultiTurnStreamItem::ToolExecutionCommitted { .. } => {
                        // TODO
                        Self(Vec::new())
                    },
                    MultiTurnStreamItem::ModelTurnRetried { .. } => {
                        // TODO
                        Self(Vec::new())
                    },
                    _ => Self(Vec::new()),
                }
            },
        }
    }

    fn from_plan_entries(entries: Vec<update_plan::PlanEntry>) -> Vec<SessionUpdate> {
        let entries = entries.into_iter().map(|e| PlanEntry::new(e).into()).collect();
        vec![SessionUpdate::Plan(Plan::new(entries))]
    }

    fn from_stream_assistant_content(
        content: StreamedAssistantContent<()>,
        tool_calls: &Mutex<HashMap<String, ToolCall>>,
    ) -> Vec<SessionUpdate> {
        match content {
            StreamedAssistantContent::Text(t) => {
                vec![SessionUpdate::AgentMessageChunk(ContentChunk::new(ContentBlock::from(
                    t.text(),
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
                    SessionUpdate::AgentThoughtChunk(ContentChunk::new(ContentBlock::from(text)))
                })
                .collect(),
            StreamedAssistantContent::ReasoningDelta { reasoning, .. } => {
                vec![SessionUpdate::AgentThoughtChunk(ContentChunk::new(ContentBlock::from(
                    reasoning,
                )))]
            },
            StreamedAssistantContent::ToolCall { tool_call, internal_call_id, .. } => {
                tool_calls.lock().insert(internal_call_id, tool_call.clone());

                let (title, content) = parse_tool_call(tool_call.clone());
                let locations = locations(tool_call.clone()).unwrap_or_default();

                let ToolFunction { name, arguments } = tool_call.function;

                let acp_tool_call = AcpToolCall::new(ToolCallId::new(tool_call.id), title)
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

                let tool_call = tool_calls.lock().remove(&internal_call_id);
                let (name, raw_input, content) = match tool_call {
                    Some(t) => {
                        let ToolFunction { name, arguments, .. } = t.function.clone();

                        let content = match name.as_str() {
                            edit::NAME | multiedit::NAME | write::NAME => parse_tool_call(t).1,
                            update_plan::NAME => {
                                // TODO: 不和谐
                                if let Ok(output) = serde_json::from_str::<
                                    update_plan::UpdatePlanOutput,
                                >(&raw_output)
                                {
                                    return Self::from_plan_entries(output.items);
                                }
                                vec![]
                            },
                            _ => {
                                let output = serde_json::from_str::<serde_json::Value>(&raw_output)
                                    .unwrap_or_else(|_| {
                                        serde_json::Value::String(raw_output.clone())
                                    });
                                vec![ToolCallContent::from(ContentBlock::from(format_tool_output(
                                    &name, output,
                                )))]
                            },
                        };
                        (Some(name), Some(arguments), Some(content))
                    },
                    None => (None, None, None),
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

fn parse_tool_call(t: ToolCall) -> (String, Vec<ToolCallContent>) {
    let ToolFunction { name, arguments, .. } = t.function;

    let default_title = format!("{name}: {arguments}");
    match name.as_str() {
        agent::NAME => serde_json::from_value::<agent::AgentArgs>(arguments)
            .map(|args| (args.title(), vec![ToolCallContent::Content(Content::new(args.prompt))]))
            .unwrap_or_else(|_| (default_title, vec![])),
        bash::NAME => serde_json::from_value::<bash::BashArgs>(arguments)
            .map(|args| (args.title(), vec![]))
            .unwrap_or_else(|_| (default_title, vec![])),
        batch::NAME => serde_json::from_value::<batch::BatchArgs>(arguments)
            .map(|args| (args.title(), vec![]))
            .unwrap_or_else(|_| (default_title, vec![])),
        glob::NAME => serde_json::from_value::<glob::GlobArgs>(arguments)
            .map(|args| (args.title(), vec![]))
            .unwrap_or_else(|_| (default_title, vec![])),
        grep::NAME => serde_json::from_value::<grep::GrepArgs>(arguments)
            .map(|args| (args.title(), vec![]))
            .unwrap_or_else(|_| (default_title, vec![])),
        lsp::NAME => serde_json::from_value::<lsp::LspArgs>(arguments)
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
        question::NAME => serde_json::from_value::<question::AskUserQuestionArgs>(arguments)
            .map(|args| (args.title(), vec![]))
            .unwrap_or_else(|_| (default_title, vec![])),
        skill::NAME => serde_json::from_value::<skill::SkillArgs>(arguments)
            .map(|args| (args.title(), vec![]))
            .unwrap_or_else(|_| (default_title, vec![])),
        update_plan::NAME => serde_json::from_value::<update_plan::UpdatePlanArgs>(arguments)
            .map(|args| (args.title(), vec![]))
            .unwrap_or_else(|_| (default_title, vec![])),
        _ => (default_title, vec![]),
    }
}

fn tool_kind(tool_name: &Option<String>) -> ToolKind {
    match tool_name {
        Some(tool_name) => match tool_name.as_str() {
            glob::NAME | read::NAME => ToolKind::Read,
            edit::NAME | multiedit::NAME | write::NAME => ToolKind::Edit,
            grep::NAME | codesearch::NAME | lsp::NAME => ToolKind::Search,
            bash::NAME | batch::NAME => ToolKind::Execute,
            webfetch::NAME | websearch::NAME => ToolKind::Fetch,
            agent::NAME | skill::NAME | question::NAME | update_plan::NAME => ToolKind::Think,
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
            .map(|args| vec![ToolCallLocation::new(args.location())])
            .ok(),
        write::NAME => serde_json::from_value::<write::WriteArgs>(arguments)
            .map(|args| vec![ToolCallLocation::new(args.location())])
            .ok(),
        edit::NAME => serde_json::from_value::<edit::EditArgs>(arguments)
            .map(|args| vec![ToolCallLocation::new(args.location())])
            .ok(),
        multiedit::NAME => serde_json::from_value::<multiedit::MultiEditArgs>(arguments)
            .map(|args| vec![ToolCallLocation::new(args.location())])
            .ok(),
        glob::NAME => serde_json::from_value::<glob::GlobArgs>(arguments)
            .ok()
            .and_then(|args| args.base_dir)
            .map(|path| vec![ToolCallLocation::new(path)]),
        _ => None,
    }
}
