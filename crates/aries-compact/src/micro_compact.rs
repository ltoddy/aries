use std::collections::HashSet;

use aries_tools::{agent, edit, multiedit, skill, update_plan, write};
use rig::message::{AssistantContent, CallId, Message, ToolResultContent, UserContent};

const TOOL_RESULT_PLACEHOLDER: &str = "[Old tool result content cleared]";
const TOOL_CALL_PLACEHOLDER: &str = "[Old tool call content cleared — file can be re-read]";
pub const KEEP_RECENT: usize = 8;

/// 保留对会话状态有控制语义的工具（Agent/UpdatePlan/Skill）。
const KEEP_TOOL_RESULT_TOOL_NAMES: &[&str; 3] = &[agent::NAME, skill::NAME, update_plan::NAME];

const COMPACTABLE_TOOL_CALL_TOOL_NAMES: &[&str; 3] = &[edit::NAME, multiedit::NAME, write::NAME];

pub fn micro_compact(messages: &mut [Message], keep_recent: usize) -> bool {
    let mut compacted = false;

    // 清理 Tool Result

    let compactable: Vec<CallId> = messages
        .iter()
        .filter_map(|m| if let Message::User { content } = m { Some(content) } else { None })
        .flatten()
        .filter_map(|c| if let UserContent::ToolResult(tr) = c { Some(tr) } else { None })
        .filter(|tr| !KEEP_TOOL_RESULT_TOOL_NAMES.contains(&tr.name.as_str()))
        .map(|tr| tr.call.to_owned())
        .collect();

    let clears: HashSet<CallId> = compactable.into_iter().rev().skip(keep_recent).collect();

    for message in messages.iter_mut() {
        if let Message::User { content } = message {
            for item in content.iter_mut() {
                if let UserContent::ToolResult(tr) = item
                    && clears.contains(&tr.call)
                {
                    *item = UserContent::tool_result(
                        tr.call.clone(),
                        tr.name.clone(),
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

    let compactable: Vec<CallId> = messages
        .iter()
        .filter_map(|m| {
            if let Message::Assistant { content, .. } = m { Some(content.iter()) } else { None }
        })
        .flatten()
        .filter_map(|c| if let AssistantContent::ToolCall(tc) = c { Some(tc) } else { None })
        .filter(|tc| COMPACTABLE_TOOL_CALL_TOOL_NAMES.contains(&tc.function.name.as_str()))
        .map(|tc| tc.id.clone())
        .collect();

    let clears: HashSet<CallId> = compactable.into_iter().rev().skip(keep_recent).collect();

    for message in messages.iter_mut() {
        if let Message::Assistant { content, .. } = message {
            for item in content.iter_mut() {
                if let AssistantContent::ToolCall(tc) = item
                    && clears.contains(&tc.id)
                {
                    *item = AssistantContent::tool_call(
                        tc.id.to_string(),
                        tc.function.name.clone(),
                        serde_json::json!({"note": TOOL_CALL_PLACEHOLDER}),
                    );
                    compacted = true;
                }
            }
        }
    }

    compacted
}
