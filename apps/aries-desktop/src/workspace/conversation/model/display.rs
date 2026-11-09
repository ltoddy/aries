use std::fmt::Debug;

use agent_client_protocol::v1::schema::{
    CompactionUpdate, ContentBlock, EmbeddedResourceResource, NoticeSeverity, PlanEntry,
    PlanEntryPriority, PlanEntryStatus, PlanUpdateContent, SessionUpdate,
};
use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateDisplay {
    title: String,
    body: String,
}

impl UpdateDisplay {
    pub fn into_parts(self) -> (String, String) {
        (self.title, self.body)
    }
}

pub fn code_block(language: &str, text: &str) -> String {
    let longest = text.split(|c| c != '`').map(str::len).max().unwrap_or(0);
    let fence = "`".repeat(longest.max(2) + 1);
    format!("{fence}{language}\n{text}\n{fence}")
}

fn json(value: &(impl Serialize + Debug)) -> String {
    match serde_json::to_string_pretty(value) {
        Ok(text) => code_block("json", &text),
        Err(error) => format!("无法解析会话更新：{error}\n\n{value:#?}"),
    }
}

fn content(block: &ContentBlock) -> String {
    match block {
        ContentBlock::Text(text) => text.text.clone(),
        ContentBlock::Image(image) => format!("图片 · {}", image.mime_type),
        ContentBlock::Audio(audio) => format!("音频 · {}", audio.mime_type),
        ContentBlock::ResourceLink(resource) => format!(
            "资源：{}\n\n{}\n\n{}",
            resource.name,
            code_block("", &resource.uri),
            resource.description.as_deref().unwrap_or_default()
        ),
        ContentBlock::Resource(resource) => match &resource.resource {
            EmbeddedResourceResource::TextResourceContents(text) => format!(
                "资源内容\n\n{}\n\n{}",
                code_block("", &text.uri),
                code_block("", &text.text)
            ),
            EmbeddedResourceResource::BlobResourceContents(blob) => {
                format!("资源内容\n\n{}\n\n{}", code_block("", &blob.uri), json(blob))
            },
            other => json(other),
        },
        other => json(other),
    }
}

fn content_list(items: &[ContentBlock]) -> String {
    let text = items.iter().map(content).collect::<Vec<_>>().join("\n\n");
    if text.is_empty() { json(&items) } else { text }
}

fn plan_entries(entries: &[PlanEntry]) -> String {
    if entries.is_empty() {
        return "计划已清空".into();
    }
    entries
        .iter()
        .map(|entry| {
            let status = match entry.status {
                PlanEntryStatus::Pending => "等待中".to_owned(),
                PlanEntryStatus::InProgress => "进行中".to_owned(),
                PlanEntryStatus::Completed => "已完成".to_owned(),
                _ => json(&entry.status),
            };
            let priority = match entry.priority {
                PlanEntryPriority::High => "high".to_owned(),
                PlanEntryPriority::Medium => "medium".to_owned(),
                PlanEntryPriority::Low => "low".to_owned(),
                _ => json(&entry.priority),
            };
            format!("- {status} · {priority} · {}", entry.content)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn plan_content(plan: &PlanUpdateContent) -> String {
    match plan {
        PlanUpdateContent::Items(plan) => {
            format!("{}\n\n{}", plan.plan_id, plan_entries(&plan.entries))
        },
        PlanUpdateContent::File(plan) => {
            format!("{}\n\n{}", plan.plan_id, code_block("", &plan.uri))
        },
        PlanUpdateContent::Markdown(plan) => format!("{}\n\n{}", plan.plan_id, plan.content),
        other => json(other),
    }
}

impl From<&SessionUpdate> for UpdateDisplay {
    fn from(update: &SessionUpdate) -> Self {
        let (title, body) = match update {
            SessionUpdate::AgentMessageChunk(chunk) => ("回复内容".into(), content(&chunk.content)),
            SessionUpdate::UserMessageChunk(chunk) => ("用户内容".into(), content(&chunk.content)),
            SessionUpdate::AgentThoughtChunk(chunk) => ("思考内容".into(), content(&chunk.content)),
            SessionUpdate::Plan(plan) => ("执行计划".into(), plan_entries(&plan.entries)),
            SessionUpdate::PlanUpdate(update) => ("计划更新".into(), plan_content(&update.plan)),
            SessionUpdate::PlanRemoved(update) => {
                ("计划移除".into(), code_block("", &update.plan_id.0))
            },
            SessionUpdate::CurrentModeUpdate(update) => {
                ("会话模式".into(), code_block("", &update.current_mode_id.0))
            },
            SessionUpdate::Notice(notice) => {
                let severity = match &notice.severity {
                    NoticeSeverity::Info => "info",
                    NoticeSeverity::Warning => "warning",
                    NoticeSeverity::Error => "error",
                    NoticeSeverity::Other(value) => value.as_str(),
                    _ => {
                        return Self {
                            title: format!("通知 · {}", notice.title),
                            body: json(notice),
                        };
                    },
                };
                (
                    format!("通知 · {}", notice.title),
                    format!("{severity}\n\n{}", notice.description.as_deref().unwrap_or_default()),
                )
            },
            SessionUpdate::CompactionUpdate(update) => return Self::from(update),
            SessionUpdate::CompactionSummaryChunk(chunk) => {
                ("压缩摘要".into(), content(&chunk.content))
            },
            SessionUpdate::SubagentUpdate(update) => {
                let mut parts = vec![code_block("", &update.session_id.0)];
                if let Some(title) = update.title.value() {
                    parts.push(title.clone());
                }
                if let Some(description) = update.description.value() {
                    parts.push(description.clone());
                }
                if let Some(state) = update.state.value() {
                    parts.push(json(state));
                }
                ("子 Agent".into(), parts.join("\n\n"))
            },
            SessionUpdate::SessionMessage(message) => {
                let mut parts = vec![code_block("", &message.message_id.0)];
                if let Some(sender) = message.sender_session_id.value() {
                    parts.push(format!("来自：{sender}"));
                }
                if let Some(recipient) = message.recipient_session_id.value() {
                    parts.push(format!("发送到：{recipient}"));
                }
                if let Some(items) = message.content.value() {
                    parts.push(content_list(items));
                }
                ("会话消息".into(), parts.join("\n\n"))
            },
            SessionUpdate::SessionMessageChunk(chunk) => (
                "会话消息".into(),
                format!("{}\n\n{}", code_block("", &chunk.message_id.0), content(&chunk.content)),
            ),
            SessionUpdate::ToolCall(_) => ("会话更新 · tool_call".into(), json(update)),
            SessionUpdate::ToolCallUpdate(_) => {
                ("会话更新 · tool_call_update".into(), json(update))
            },
            SessionUpdate::AvailableCommandsUpdate(_) => {
                ("会话更新 · available_commands_update".into(), json(update))
            },
            SessionUpdate::ConfigOptionUpdate(_) => {
                ("会话更新 · config_option_update".into(), json(update))
            },
            SessionUpdate::SessionInfoUpdate(_) => {
                ("会话更新 · session_info_update".into(), json(update))
            },
            SessionUpdate::UsageUpdate(_) => ("会话更新 · usage_update".into(), json(update)),
            _ => ("会话更新".into(), json(update)),
        };
        Self { title, body }
    }
}

impl From<&CompactionUpdate> for UpdateDisplay {
    fn from(update: &CompactionUpdate) -> Self {
        let mut parts = Vec::new();
        if let Some(summary) = update.summary.value() {
            parts.extend(summary.iter().map(content));
        }
        let title = parts.join("\n\n");
        if let Some(error) = update.error.value() {
            parts.push(json(error));
        }
        Self { title, body: parts.join("\n\n") }
    }
}
