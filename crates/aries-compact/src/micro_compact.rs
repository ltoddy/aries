use std::collections::HashSet;

use aries_tools::{agent, edit, multiedit, question, skill, update_plan, write};
use rig::message::{AssistantContent, Message, ToolCall, ToolResultContent, UserContent};

const TOOL_RESULT_PLACEHOLDER: &str = "[Old tool result content cleared]";
const TOOL_CALL_PLACEHOLDER: &str = "[Old tool call content cleared — file can be re-read]";
pub const KEEP_RECENT: usize = 8;

/// 保留对会话状态有控制语义的工具（Agent/UpdatePlan/Question/Skill），
const KEEP_TOOL_RESULT_TOOL_NAMES: &[&str; 4] =
    &[agent::NAME, question::NAME, skill::NAME, update_plan::NAME];

const COMPACTABLE_TOOL_CALL_TOOL_NAMES: &[&str; 3] = &[edit::NAME, multiedit::NAME, write::NAME];

pub fn micro_compact(messages: &mut [Message], keep_recent: usize) -> bool {
    let mut compacted = false;

    // 清理 Tool Result

    let compactable = messages
        .iter()
        .filter_map(|m| if let Message::User { content } = m { Some(content) } else { None })
        .flatten()
        .filter_map(|c| if let UserContent::ToolResult(tr) = c { Some(tr) } else { None })
        .filter(|tr| !KEEP_TOOL_RESULT_TOOL_NAMES.contains(&tr.name.as_str()))
        .map(|tr| tr.call.to_owned())
        .collect::<Vec<_>>();

    let clears = compactable.into_iter().rev().skip(keep_recent).collect::<HashSet<_>>();

    for message in messages.iter_mut() {
        if let Message::User { content } = message {
            for item in content.iter_mut() {
                if let UserContent::ToolResult(tr) = item
                    && clears.contains(tr.call.as_str())
                {
                    *item = UserContent::tool_result(
                        tr.call.as_str(),
                        &tr.name,
                        vec![ToolResultContent::json(
                            serde_json::json!({"note": TOOL_RESULT_PLACEHOLDER}),
                        )],
                    );
                    compacted = true;
                }
            }
        }
    }

    // 清理 Tool Call

    let compactable = messages
        .iter()
        .filter_map(|m| {
            if let Message::Assistant { content, .. } = m { Some(content.iter()) } else { None }
        })
        .flatten()
        .filter_map(|c| if let AssistantContent::ToolCall(tc) = c { Some(tc) } else { None })
        .filter(|tc| COMPACTABLE_TOOL_CALL_TOOL_NAMES.contains(&tc.function.name.as_str()))
        .map(|tc| tc.id.clone())
        .collect::<Vec<_>>();

    let clears: HashSet<_> = compactable.into_iter().rev().skip(keep_recent).collect();

    for message in messages.iter_mut() {
        if let Message::Assistant { content, .. } = message {
            for item in content.iter_mut() {
                if let AssistantContent::ToolCall(ToolCall { id, function, .. }) = item
                    && clears.contains(id)
                {
                    *item = AssistantContent::tool_call(
                        id.as_str(),
                        &function.name,
                        serde_json::json!({"note": TOOL_CALL_PLACEHOLDER}),
                    );
                    compacted = true;
                }
            }
        }
    }

    compacted
}
