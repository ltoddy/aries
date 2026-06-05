use std::collections::HashMap;
use std::io::Write;

use aries_core::event::AgentEvent;
use aries_core::tools;
use itertools::Itertools;
use rig_core::agent::MultiTurnStreamItem;
use rig_core::message::ToolResultContent;
use rig_core::streaming::{StreamedAssistantContent, StreamedUserContent};

use crate::theme::Theme;

pub fn print_agent_event(
    event: AgentEvent,
    theme: Theme,
    tool_names: &mut HashMap<String, String>,
) {
    let prefix = if event.main {
        String::new()
    } else {
        format!("{} ", theme.magenta_text(&format!("[{}]", event.name)))
    };

    match event.item {
        MultiTurnStreamItem::StreamAssistantItem(content) => match content {
            StreamedAssistantContent::Text(text) => {
                if !text.text.is_empty() {
                    print!("{}{}", prefix, text.text);
                    let _ = std::io::stdout().flush();
                }
            },
            StreamedAssistantContent::ToolCall { tool_call, internal_call_id } => {
                tool_names.insert(internal_call_id, tool_call.function.name.clone());

                let args = tool_call.function.arguments.to_string();
                let (call_str, rest) =
                    format_tool_call_args(&tool_call.function.name, &args, &theme);
                println!("\n{}{} {}", prefix, theme.cyan_text("•"), call_str);
                if let Some(rest) = rest {
                    println!("{}", rest);
                }
            },
            _ => {},
        },
        MultiTurnStreamItem::StreamUserItem(StreamedUserContent::ToolResult {
            tool_result,
            internal_call_id,
        }) => {
            let tool_name = tool_names.remove(&internal_call_id).unwrap_or_default();
            let raw = tool_result
                .content
                .iter()
                .filter_map(|c| match c {
                    ToolResultContent::Text(text) => Some(text.text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n");
            let formatted = format_tool_result_output(&tool_name, &raw, theme);
            println!("{}{}", prefix, formatted);
        },
        MultiTurnStreamItem::FinalResponse(res) if event.main => {
            display_token_usage(&res.usage(), &theme);
        },
        _ => {},
    }
}

pub fn format_tool_call_args(
    tool_name: &str,
    args: &str,
    theme: &Theme,
) -> (String, Option<String>) {
    if !is_known_tool(tool_name) {
        return format_unknown_call(tool_name, args, theme);
    }

    let (first, rest) = tools::format_tool_args(tool_name, args);

    (format!("{} {}", theme.cyan_text(tool_name), theme.yellow_text(&first)), rest)
}

fn is_known_tool(tool_name: &str) -> bool {
    matches!(
        tool_name,
        tools::agent::NAME
            | tools::bash::NAME
            | tools::batch::NAME
            | tools::codesearch::NAME
            | tools::edit::NAME
            | tools::glob::NAME
            | tools::grep::NAME
            | tools::ls::NAME
            | tools::lsp::NAME
            | tools::multiedit::NAME
            | tools::question::NAME
            | tools::read::NAME
            | tools::skill::NAME
            | tools::webfetch::NAME
            | tools::websearch::NAME
            | tools::write::NAME
    )
}

pub fn format_tool_result_output(tool_name: &str, result: &str, theme: Theme) -> String {
    let output = tools::format_tool_output(tool_name, result);
    let output = if output.is_empty() { "No output".to_string() } else { output };

    let _ = tool_name;
    theme.dimmed(&preview(output)).to_string()
}

pub fn display_token_usage(usage: &rig_core::completion::Usage, theme: &Theme) {
    println!(
        "\n\n{} total={} input={} (cached={}) output={}",
        theme.dimmed("Token usage:"),
        theme.dimmed(&usage.total_tokens.to_string()),
        theme.dimmed(&usage.input_tokens.to_string()),
        theme.dimmed(&usage.cached_input_tokens.to_string()),
        theme.dimmed(&usage.output_tokens.to_string())
    );
}

pub fn format_unknown_call(tool_name: &str, args: &str, theme: &Theme) -> (String, Option<String>) {
    let args_str = serde_json::from_str::<serde_json::Value>(args)
        .and_then(|value| serde_json::to_string_pretty(&value))
        .unwrap_or_else(|_| args.to_string());

    (
        format!("{} {}", theme.cyan_text(tool_name), theme.yellow_text("(unknown tool)")),
        Some(args_str),
    )
}

pub fn preview(content: impl Into<String>) -> String {
    const MAX_LINES: usize = 5;

    let content = content.into();
    let lines: Vec<_> = content.lines().map(|line| format!("| {line}")).collect();
    let len = lines.len();

    if len > MAX_LINES {
        let preview = lines[..MAX_LINES].iter().join("\n");
        format!("{}\n+ ... ({} more lines truncated)", preview, len - MAX_LINES)
    } else {
        lines.join("\n")
    }
}
