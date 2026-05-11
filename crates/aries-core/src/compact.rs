use std::collections::HashMap;

use rig::OneOrMany;
use rig::message::{AssistantContent, Message, ToolResultContent, UserContent};
use serde_json::Value;

pub const KEEP_RECENT_TOOLS: usize = 5;
pub const TOKEN_THRESHOLD: u64 = 80_000;

pub fn micro_compact(messages: &mut [Message]) {
    let tool_map = build_tool_name_map(messages);

    for message in messages.iter_mut().rev().skip(KEEP_RECENT_TOOLS) {
        match message {
            Message::User { content } => compact_tool_result(content, &tool_map),
            Message::Assistant { content, .. } => compact_tool_call(content, &tool_map),
            _ => {},
        }
    }
}

fn compact_tool_result(content: &mut OneOrMany<UserContent>, tool_map: &HashMap<String, String>) {
    for item in content.iter_mut() {
        let UserContent::ToolResult(tool_result) = item else { continue };

        let Some(tool_name) = tool_map.get(&tool_result.id) else { continue };
        let placeholder = format!("[Previous tool result: {tool_name}]");

        *item = UserContent::tool_result(
            tool_result.id.clone(),
            OneOrMany::one(ToolResultContent::text(placeholder)),
        );
    }
}

fn compact_tool_call(
    content: &mut OneOrMany<AssistantContent>,
    tool_map: &HashMap<String, String>,
) {
    for item in content.iter_mut() {
        let AssistantContent::ToolCall(tool_call) = item else { continue };

        let Some(tool_name) = tool_map.get(&tool_call.id) else { continue };
        let placeholder = format!("[Previous tool call: {tool_name}]");

        *item = AssistantContent::tool_call(
            tool_call.id.clone(),
            tool_name,
            Value::String(placeholder),
        );
    }
}

fn build_tool_name_map(messages: &[Message]) -> HashMap<String, String> {
    messages
        .iter()
        .filter_map(|msg| match msg {
            Message::Assistant { content, .. } => Some(content.iter()),
            _ => None,
        })
        .flatten()
        .filter_map(|c| match c {
            AssistantContent::ToolCall(tc) => Some((tc.id.clone(), tc.function.name.clone())),
            _ => None,
        })
        .collect()
}
