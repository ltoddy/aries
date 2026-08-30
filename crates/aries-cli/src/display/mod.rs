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
use aries_tools::lsp::LspArgs;
use aries_tools::monitor::MonitorArgs;
use aries_tools::multiedit::MultiEditArgs;
use aries_tools::question::AskUserQuestionArgs;
use aries_tools::read::ReadArgs;
use aries_tools::skill::SkillArgs;
use aries_tools::task_output::TaskOutputArgs;
use aries_tools::task_stop::TaskStopArgs;
use aries_tools::update_plan::UpdatePlanArgs;
use aries_tools::webfetch::WebFetchArgs;
use aries_tools::websearch::WebSearchArgs;
use aries_tools::write::WriteArgs;
use aries_tools::{
    agent, bash, batch, codesearch, edit, glob, grep, lsp, monitor, multiedit, question, read,
    skill, task_output, task_stop, update_plan, webfetch, websearch, write,
};
use colored::Colorize;
use rig::agent::MultiTurnStreamItem;
use rig::message::ToolResultContent;
use rig::streaming::{StreamedAssistantContent, StreamedUserContent};

use crate::text;

pub fn print_agent_event(event: AgentEvent, tool_names: &mut HashMap<String, String>) {
    match event {
        AgentEvent::Notification(text) => println!("{text}"),
        AgentEvent::StreamItem(stream_item) => match *stream_item {
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
                    let (first, rest) = format_tool_call_args(&tool_call.function.name, &args);
                    println!("\n{} {}", "•".cyan(), first);
                    if let Some(rest) = rest {
                        for line in rest.lines() {
                            if let Some(content) = line.strip_prefix("- ") {
                                println!("- {}", content.red());
                            } else if let Some(content) = line.strip_prefix("+ ") {
                                println!("+ {}", content.green());
                            } else {
                                println!("{line}");
                            }
                        }
                    }
                },
                _ => {},
            },
            MultiTurnStreamItem::StreamUserItem(StreamedUserContent::ToolResult {
                tool_result,
                internal_call_id,
            }) => {
                let tool_name = tool_names.remove(&internal_call_id).unwrap_or_default();
                for result in tool_result.content {
                    if let ToolResultContent::Json { value } = result {
                        let formatted = format_tool_result_output(&tool_name, value);
                        println!("{formatted}");
                    }
                }
            },
            MultiTurnStreamItem::FinalResponse(res) => {
                display_token_usage(&res.usage());
            },
            MultiTurnStreamItem::CompletionCall(_) => {},
            MultiTurnStreamItem::ToolExecutionCommitted { .. } => {},
            MultiTurnStreamItem::ModelTurnRetried { .. } => {},
        },
        AgentEvent::AwaitingUserInput { .. } => {},
        AgentEvent::SessionInfoUpdate { .. } => {},
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
        lsp::NAME => LspArgs::render_args(args),
        monitor::NAME => MonitorArgs::render_args(args),
        multiedit::NAME => MultiEditArgs::render_args(args),
        question::NAME => AskUserQuestionArgs::render_args(args),
        read::NAME => ReadArgs::render_args(args),
        skill::NAME => SkillArgs::render_args(args),
        task_output::NAME => TaskOutputArgs::render_args(args).map(|title| (title, None)),
        task_stop::NAME => TaskStopArgs::render_args(args).map(|title| (title, None)),
        update_plan::NAME => UpdatePlanArgs::render_args(args),
        webfetch::NAME => WebFetchArgs::render_args(args),
        websearch::NAME => WebSearchArgs::render_args(args),
        write::NAME => WriteArgs::render_args(args),
        _ => Ok((args.to_string(), None)),
    };

    let (first, rest) = result.unwrap_or_else(|_| (args.to_string(), None));

    (format!("{} {}", tool_name.cyan(), first.yellow()), rest)
}

pub fn format_tool_result_output(tool_name: &str, value: serde_json::Value) -> String {
    let output = aries_tools::format_tool_output(tool_name, value);
    let output = if output.is_empty() { "No output".to_string() } else { output };

    text::preview(output).dimmed().to_string()
}

pub fn display_token_usage(usage: &rig::completion::Usage) {
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
