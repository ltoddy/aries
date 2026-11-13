use std::io::Write;

use aries_communicate::AgentEvent;
use aries_runtime::tools::agent::AgentArgs;
use aries_runtime::tools::bash::BashArgs;
use aries_runtime::tools::batch::BatchArgs;
use aries_runtime::tools::codesearch::CodeSearchArgs;
use aries_runtime::tools::edit::EditArgs;
use aries_runtime::tools::glob::GlobArgs;
use aries_runtime::tools::grep::GrepArgs;
use aries_runtime::tools::monitor::MonitorArgs;
use aries_runtime::tools::multiedit::MultiEditArgs;
use aries_runtime::tools::read::ReadArgs;
use aries_runtime::tools::skill::SkillArgs;
use aries_runtime::tools::task_output::TaskOutputArgs;
use aries_runtime::tools::task_stop::TaskStopArgs;
use aries_runtime::tools::update_plan::UpdatePlanArgs;
use aries_runtime::tools::webfetch::WebFetchArgs;
use aries_runtime::tools::websearch::WebSearchArgs;
use aries_runtime::tools::write::WriteArgs;
use aries_runtime::tools::{
    agent, bash, batch, codesearch, edit, glob, grep, monitor, multiedit, read, skill, task_output,
    task_stop, update_plan, webfetch, websearch, write,
};
use colored::Colorize;
use rig::agent::MultiTurnStreamItem;
use rig::streaming::{Item, StreamEvent};

pub fn print_agent_event(event: AgentEvent) {
    match event {
        AgentEvent::Notification(text) => println!("{text}"),
        AgentEvent::StreamItem(stream_item) => match *stream_item {
            MultiTurnStreamItem::StreamAssistantItem(content) => match content {
                Item::Event(event) => match event {
                    StreamEvent::Start { part: _, kind: _, name: _ } => {},
                    StreamEvent::Text { part: _, text } => {
                        print!("{text}");
                        let _ = std::io::stdout().flush();
                    },
                    StreamEvent::Reasoning { part: _, text } => {
                        print!("{}", text.dimmed());
                        let _ = std::io::stdout().flush();
                    },
                    StreamEvent::Arguments { part: _, json: _ } => {},
                    StreamEvent::End { part: _, content: _ } => println!(),
                },
                Item::Unknown(_payload) => {},
            },
            MultiTurnStreamItem::ToolCall { tool_call } => {
                let args = serde_json::Value::Object(tool_call.function.arguments).to_string();
                let (first, rest) = format_tool_call_args(&tool_call.function.name, &args);
                println!("{} {}", "•".cyan(), first);
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
            MultiTurnStreamItem::ToolExecutionCommitted { tool_call: _ } => {},
            MultiTurnStreamItem::ToolResult { tool_result: _ } => {},
            MultiTurnStreamItem::CompletionCall(_completion_call) => {},
            MultiTurnStreamItem::ModelTurnRetried { turn: _ } => {},
            MultiTurnStreamItem::FinalResponse(res) => {
                display_token_usage(&res.usage());
            },
            _ => {},
        },
        AgentEvent::Compaction(compaction) => println!("{}", compaction.to_string().dimmed()),
        AgentEvent::SessionInfoUpdate { title: _, updated_at: _ } => {},
        _ => {
            // TODO
        },
    }
}

pub fn format_tool_call_args(
    tool_name: impl AsRef<str>,
    args: impl AsRef<str>,
) -> (String, Option<String>) {
    let tool_name = tool_name.as_ref();
    let args = args.as_ref();
    if !aries_runtime::tools::is_builtin_tool(tool_name) {
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
        monitor::NAME => MonitorArgs::render_args(args),
        multiedit::NAME => MultiEditArgs::render_args(args),
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

pub fn display_token_usage(usage: &rig::completion::Usage) {
    let total_tokens = usage.total_tokens.unwrap_or_default();
    let input_tokens = usage.input_tokens.unwrap_or_default();
    let cached_input_tokens = usage.cached_input_tokens.unwrap_or_default();
    let output_tokens = usage.output_tokens.unwrap_or_default();

    println!(
        "{} total={} input={} (cached={}) output={}",
        "Token usage:".dimmed(),
        total_tokens.to_string().dimmed(),
        input_tokens.to_string().dimmed(),
        cached_input_tokens.to_string().dimmed(),
        output_tokens.to_string().dimmed()
    );
}

pub fn format_unknown_call(
    tool_name: impl AsRef<str>,
    args: impl AsRef<str>,
) -> (String, Option<String>) {
    let tool_name = tool_name.as_ref();
    let args = args.as_ref();
    let args_str = serde_json::from_str::<serde_json::Value>(args)
        .and_then(|value| serde_json::to_string_pretty(&value))
        .unwrap_or_else(|_| args.to_string());

    (format!("{} {}", tool_name.cyan(), "(unknown tool)".yellow()), Some(args_str))
}
