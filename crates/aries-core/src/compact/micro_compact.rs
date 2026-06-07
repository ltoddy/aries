use std::collections::{HashMap, HashSet};

use rig_core::OneOrMany;
use rig_core::message::{
    AssistantContent, Message, ToolCall, ToolResult, ToolResultContent, UserContent,
};

use crate::tools::{
    bash, batch, codesearch, edit, glob, grep, ls, lsp, multiedit, read, webfetch, websearch, write,
};

const PLACEHOLDER: &str = "[Old tool result content cleared]";
const KEEP_RECENT: usize = 5;

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
                        OneOrMany::one(ToolResultContent::text(PLACEHOLDER.to_owned())),
                    );
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

/// - 只清空"信息密集、可重新获取"的工具结果（Read/Bash/Grep/Glob/Edit/Write/Web*…）。
/// - 保留对会话状态有控制语义的工具（Agent/UpdatePlan/Question/Skill）。
const COMPACTABLE_TOOL_NAMES: &[&str; 13] = &[
    bash::NAME,
    batch::NAME,
    codesearch::NAME,
    edit::NAME,
    glob::NAME,
    grep::NAME,
    ls::NAME,
    lsp::NAME,
    multiedit::NAME,
    read::NAME,
    webfetch::NAME,
    websearch::NAME,
    write::NAME,
];

#[inline]
fn is_compactable_tool_result(tr: &ToolResult, tools: &HashMap<String, String>) -> bool {
    tools.get(&tr.id).is_some_and(|name| COMPACTABLE_TOOL_NAMES.contains(&name.as_str()))
}
