use std::collections::{HashMap, HashSet};

use aries_tools::{
    bash, batch, codesearch, edit, glob, grep, lsp, multiedit, read, webfetch, websearch, write,
};
use rig_core::OneOrMany;
use rig_core::message::{
    AssistantContent, Message, ToolCall, ToolResult, ToolResultContent, UserContent,
};

const TOOL_RESULT_PLACEHOLDER: &str = "[Old tool result content cleared]";
const TOOL_CALL_PLACEHOLDER: &str = "[Old tool call content cleared — file can be re-read]";
const KEEP_RECENT: usize = 8;

pub fn micro_compact(messages: &mut [Message]) {
    let tools = build_tools(messages);

    let compactable = messages
        .iter()
        .filter_map(|m| if let Message::User { content } = m { Some(content.iter()) } else { None })
        .flatten()
        .filter_map(|c| if let UserContent::ToolResult(tr) = c { Some(tr) } else { None })
        .filter(|tr| is_compactable_tool_result(tr, &tools))
        .map(|tr| tr.id.clone())
        .collect::<Vec<_>>();

    let clears = compactable.into_iter().rev().skip(KEEP_RECENT).collect::<HashSet<_>>();

    for message in messages.iter_mut() {
        if let Message::User { content } = message {
            for item in content.iter_mut() {
                if let UserContent::ToolResult(tr) = item
                    && clears.contains(&tr.id)
                {
                    *item = UserContent::tool_result(
                        tr.id.clone(),
                        OneOrMany::one(ToolResultContent::text(TOOL_RESULT_PLACEHOLDER.to_owned())),
                    );
                }
            }
        }
    }

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

    let clears: HashSet<_> = compactable.into_iter().rev().skip(KEEP_RECENT).collect();

    for message in messages.iter_mut() {
        if let Message::Assistant { content, .. } = message {
            for item in content.iter_mut() {
                if let AssistantContent::ToolCall(ToolCall { id, call_id, function, .. }) = item
                    && clears.contains(id)
                {
                    *item = match call_id {
                        Some(call_id) => AssistantContent::tool_call_with_call_id(
                            id.to_owned(),
                            call_id.to_owned(),
                            &function.name,
                            serde_json::Value::String(String::from(TOOL_CALL_PLACEHOLDER)),
                        ),
                        None => AssistantContent::tool_call(
                            id.to_owned(),
                            &function.name,
                            serde_json::Value::String(String::from(TOOL_CALL_PLACEHOLDER)),
                        ),
                    };
                }
            }
        }
    }
}

#[inline]
fn build_tools(messages: &[Message]) -> HashMap<String, String> {
    messages
        .iter()
        .filter_map(|m| {
            if let Message::Assistant { content, .. } = m { Some(content.iter()) } else { None }
        })
        .flatten()
        .filter_map(|c| {
            if let AssistantContent::ToolCall(ToolCall { id, function, .. }) = c {
                Some((id.clone(), function.name.clone()))
            } else {
                None
            }
        })
        .collect()
}

const COMPACTABLE_TOOL_CALL_TOOL_NAMES: &[&str; 3] = &[edit::NAME, multiedit::NAME, write::NAME];

/// - 只清空"信息密集、可重新获取"的工具结果（Read/Bash/Grep/Glob/Edit/Write/Web*…）。
/// - 保留对会话状态有控制语义的工具（Agent/UpdatePlan/Question/Skill）。
const COMPACTABLE_TOOL_RESULT_TOOL_NAMES: &[&str; 12] = &[
    bash::NAME,
    batch::NAME,
    codesearch::NAME,
    edit::NAME,
    glob::NAME,
    grep::NAME,
    lsp::NAME,
    multiedit::NAME,
    read::NAME,
    webfetch::NAME,
    websearch::NAME,
    write::NAME,
];

#[inline]
fn is_compactable_tool_result(tr: &ToolResult, tools: &HashMap<String, String>) -> bool {
    tools
        .get(&tr.id)
        .is_some_and(|name| COMPACTABLE_TOOL_RESULT_TOOL_NAMES.contains(&name.as_str()))
}
