use std::collections::HashMap;
use std::io::Write;

use aries_event::AgentEvent;
use aries_tools::agent::AgentArgs;
use aries_tools::bash::BashArgs;
use aries_tools::batch::BatchArgs;
use aries_tools::codesearch::CodeSearchArgs;
use aries_tools::edit::EditArgs;
use aries_tools::glob::GlobArgs;
use aries_tools::grep::GrepArgs;
use aries_tools::ls::LsArgs;
use aries_tools::lsp::LspArgs;
use aries_tools::multiedit::MultiEditArgs;
use aries_tools::question::AskUserQuestionArgs;
use aries_tools::read::ReadArgs;
use aries_tools::skill::SkillArgs;
use aries_tools::update_plan::UpdatePlanArgs;
use aries_tools::webfetch::WebFetchArgs;
use aries_tools::websearch::WebSearchArgs;
use aries_tools::write::WriteArgs;
use aries_tools::{
    agent, bash, batch, codesearch, edit, glob, grep, ls, lsp, multiedit, question, read, skill,
    update_plan, webfetch, websearch, write,
};
use colored::Colorize;
use rig_agent::agent::MultiTurnStreamItem;
use rig_core::message::ToolResultContent;
use rig_core::streaming::{StreamedAssistantContent, StreamedUserContent};

use crate::text;

pub fn print_agent_event(event: AgentEvent, tool_names: &mut HashMap<String, String>) {
    match event.stream_item {
        MultiTurnStreamItem::StreamAssistantItem(content) => match content {
            StreamedAssistantContent::Text(text) => {
                if !text.text.is_empty() {
                    print!("{}", text.text);
                    let _ = std::io::stdout().flush();
                }
            },
            StreamedAssistantContent::ToolCall { tool_call, internal_call_id } => {
                tool_names.insert(internal_call_id, tool_call.function.name.clone());

                let args = tool_call.function.arguments.to_string();
                let (call_str, rest) = format_tool_call_args(&tool_call.function.name, &args);
                println!("\n{} {}", "•".cyan(), call_str);
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
            let formatted = format_tool_result_output(&tool_name, &raw);
            println!("{formatted}");
        },
        MultiTurnStreamItem::FinalResponse(res) if event.main => {
            display_token_usage(&res.usage());
        },
        MultiTurnStreamItem::CompletionCall(_) => {},
        MultiTurnStreamItem::FinalResponse(_) => {},
        MultiTurnStreamItem::ToolExecutionCommitted { .. } => {},
        MultiTurnStreamItem::ModelTurnRetried { .. } => {},
        _ => {},
    }
}

pub fn format_tool_call_args(tool_name: &str, args: &str) -> (String, Option<String>) {
    if !aries_tools::is_builtin_tool(tool_name) {
        return format_unknown_call(tool_name, args);
    }

    let result = match tool_name {
        agent::NAME => AgentArgs::render_args(args),
        bash::NAME => BashArgs::render_args(args),
        batch::NAME => BatchArgs::render_args(args),
        codesearch::NAME => CodeSearchArgs::render_args(args),
        edit::NAME => EditArgs::render_args(args),
        glob::NAME => GlobArgs::render_args(args),
        grep::NAME => GrepArgs::render_args(args),
        ls::NAME => LsArgs::render_args(args),
        lsp::NAME => LspArgs::render_args(args),
        multiedit::NAME => MultiEditArgs::render_args(args),
        question::NAME => AskUserQuestionArgs::render_args(args),
        read::NAME => ReadArgs::render_args(args),
        skill::NAME => SkillArgs::render_args(args),
        update_plan::NAME => UpdatePlanArgs::render_args(args),
        webfetch::NAME => WebFetchArgs::render_args(args),
        websearch::NAME => WebSearchArgs::render_args(args),
        write::NAME => WriteArgs::render_args(args),
        _ => Ok((args.to_string(), None)),
    };

    let (first, rest) = result.unwrap_or_else(|_| (args.to_string(), None));

    (format!("{} {}", tool_name.cyan(), first.yellow()), rest)
}

pub fn format_tool_result_output(tool_name: &str, result: &str) -> String {
    let output = aries_tools::tools::format_tool_output(tool_name, result);
    let output = if output.is_empty() { "No output".to_string() } else { output };

    let _ = tool_name;
    text::preview(output).dimmed().to_string()
}

pub fn display_token_usage(usage: &rig_core::completion::Usage) {
    println!(
        "\n\n{} total={} input={} (cached={}) output={}",
        "Token usage:".dimmed(),
        usage.total_tokens.to_string().dimmed(),
        usage.input_tokens.to_string().dimmed(),
        usage.cached_input_tokens.to_string().dimmed(),
        usage.output_tokens.to_string().dimmed()
    );
}

pub fn format_unknown_call(tool_name: &str, args: &str) -> (String, Option<String>) {
    let args_str = serde_json::from_str::<serde_json::Value>(args)
        .and_then(|value| serde_json::to_string_pretty(&value))
        .unwrap_or_else(|_| args.to_string());

    (format!("{} {}", tool_name.cyan(), "(unknown tool)".yellow()), Some(args_str))
}
