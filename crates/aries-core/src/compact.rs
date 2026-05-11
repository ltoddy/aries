use std::collections::HashMap;

use rig::OneOrMany;
use rig::completion::{AssistantContent, Message};
use rig::message::{ToolResultContent, UserContent};

pub const KEEP_RECENT_TOOL_RESULTS: usize = 5;

pub fn micro_compact(messages: &mut [Message]) {
    let tool_map = build_tool_name_map(messages);

    for message in messages.iter_mut().rev().skip(KEEP_RECENT_TOOL_RESULTS) {
        let Message::User { content } = message else { continue };

        for item in content.iter_mut() {
            let UserContent::ToolResult(tool_result) = item else { continue };

            let Some(tool_name) = tool_map.get(&tool_result.id) else { continue };
            let placeholder = format!("[Previous: used {tool_name}]");

            *item = UserContent::tool_result(
                tool_result.id.clone(),
                OneOrMany::one(ToolResultContent::text(placeholder)),
            );
        }
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
