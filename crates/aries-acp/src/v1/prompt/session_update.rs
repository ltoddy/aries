use agent_client_protocol::schema::v1::{
    Content, ContentBlock, ContentChunk, Diff, Plan, SessionInfoUpdate, SessionUpdate,
    ToolCall as AcpToolCall, ToolCallContent, ToolCallId, ToolCallLocation, ToolCallStatus,
    ToolCallUpdate, ToolCallUpdateFields, ToolKind, UsageUpdate,
};
use aries_event::AgentEvent;
use aries_tools::{
    agent, bash, batch, codesearch, edit, glob, grep, lsp, monitor, multiedit, read, skill,
    task_output, task_stop, update_plan, webfetch, websearch, write,
};
use rig::agent::{MultiTurnStreamItem, PromptResponse};
use rig::message::{ToolCall, ToolFunction, ToolName};
use rig::streaming::{Item, StreamEvent, StreamedUserContent};

use super::plan::PlanEntry;

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
                MultiTurnStreamItem::StreamUserItem(v) => Self(Self::from_stream_user_content(v)),
                MultiTurnStreamItem::CompletionCall(_) => Default::default(),
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
            },
            AgentEvent::SessionInfoUpdate { title, updated_at } => {
                Self(vec![SessionUpdate::SessionInfoUpdate(
                    SessionInfoUpdate::new().title(title).updated_at(updated_at),
                )])
            },
        }
    }

    fn from_stream_assistant_content(content: Item<StreamEvent>) -> Vec<SessionUpdate> {
        match content {
            Item::Event(event) => match event {
                StreamEvent::Start { part: _, kind: _ } => {},
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
                    rig::message::AssistantContent::Text(_text) => {},
                    rig::message::AssistantContent::ToolCall(tool_call) => {
                        return vec![Self::from_tool_call(tool_call)];
                    },
                    rig::message::AssistantContent::Reasoning(_reasoning) => {},
                    rig::message::AssistantContent::Image(_image) => {},
                },
            },
            Item::Unknown(_payload) => {},
        }

        vec![]
    }

    fn from_stream_user_content(content: StreamedUserContent) -> Vec<SessionUpdate> {
        match content {
            StreamedUserContent::ToolResult { tool_result } => {
                let raw_output =
                    tool_result.content.first().and_then(|content| content.as_json()).cloned();

                if tool_result.name.as_str() == update_plan::NAME
                    && let Some(output) = raw_output.clone()
                    && let Ok(entries) =
                        serde_json::from_value::<Vec<update_plan::PlanEntry>>(output)
                {
                    return vec![Self::from_plan_entries(entries)];
                }

                let fields = ToolCallUpdateFields::new()
                    .status(ToolCallStatus::Completed)
                    .raw_output(raw_output);

                vec![SessionUpdate::ToolCallUpdate(ToolCallUpdate::new(
                    ToolCallId::new(tool_result.call.to_string()),
                    fields,
                ))]
            },
        }
    }

    fn from_plan_entries(entries: Vec<update_plan::PlanEntry>) -> SessionUpdate {
        let entries = entries.into_iter().map(|e| PlanEntry::new(e).into()).collect();
        let plan = Plan::new(entries);
        SessionUpdate::Plan(plan)
    }

    fn from_prompt_response(res: PromptResponse) -> Vec<SessionUpdate> {
        let usage = res.usage();

        let completions = res.completion_calls.len();
        let input_tokens = usage.input_tokens.unwrap_or_default();
        let cached_input_tokens = usage.cached_input_tokens.unwrap_or_default();
        let output_tokens = usage.output_tokens.unwrap_or_default();
        let total_tokens = usage.total_tokens.unwrap_or_default();
        let reasoning_tokens = usage.reasoning_tokens.unwrap_or_default();

        let text = format!(
            "\n\nCompletion({}) - This turn token usage: input tokens = {} (cached = {}), output tokens = {}, total tokens = {}, reasoning tokens = {}",
            completions,
            input_tokens,
            cached_input_tokens,
            output_tokens,
            total_tokens,
            reasoning_tokens,
        );
        let mut updates =
            vec![SessionUpdate::AgentMessageChunk(ContentChunk::new(ContentBlock::from(text)))];

        if let Some(completion) = res.completion_calls.last() {
            let total_tokens = completion.usage.total_tokens.unwrap_or_default();

            updates.push(SessionUpdate::UsageUpdate(UsageUpdate::new(total_tokens, 0)));
        }
        updates
    }

    fn from_tool_call(tool_call: ToolCall) -> SessionUpdate {
        let (title, content) = parse_tool_call(tool_call.clone());
        let locations = locations(&tool_call.function);
        let ToolFunction { name, arguments, .. } = tool_call.function;
        let kind = tool_kind(&name);

        let tc = AcpToolCall::new(ToolCallId::new(tool_call.id.to_string()), &title)
            .kind(kind)
            .status(ToolCallStatus::InProgress)
            .content(content)
            .locations(locations)
            .raw_input(arguments);

        SessionUpdate::ToolCall(tc)
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

fn tool_kind(tool_name: &ToolName) -> ToolKind {
    match tool_name.as_str() {
        glob::NAME | read::NAME | task_output::NAME => ToolKind::Read,
        edit::NAME | multiedit::NAME | write::NAME => ToolKind::Edit,
        grep::NAME | codesearch::NAME | lsp::NAME => ToolKind::Search,
        bash::NAME | batch::NAME | monitor::NAME | task_stop::NAME => ToolKind::Execute,
        webfetch::NAME | websearch::NAME => ToolKind::Fetch,
        agent::NAME | skill::NAME | update_plan::NAME => ToolKind::Think,
        _ => ToolKind::Other,
    }
}

fn locations(t: &ToolFunction) -> Vec<ToolCallLocation> {
    let arguments = t.arguments.clone();

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
