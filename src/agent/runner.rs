use std::io::Write;

use anyhow::Result;
use colored::Colorize;
use futures::StreamExt;
use rig::agent::{Agent, MultiTurnStreamItem};
use rig::completion::Message;
use rig::message::Text;
use rig::providers::deepseek;
use rig::streaming::{StreamedAssistantContent, StreamingPrompt};
use serde_json::Value;

fn format_tool_args(tool_name: &str, args: &Value) -> String {
    match tool_name {
        "read_file" => {
            let path = args.get("file_path").and_then(|v| v.as_str()).unwrap_or("?");
            format!("{} {}", tool_name.cyan(), path.yellow())
        },
        "write_file" => {
            let path = args.get("file_path").and_then(|v| v.as_str()).unwrap_or("?");
            format!("{} {}", tool_name.cyan(), path.yellow())
        },
        "shell_command" => {
            let cmd = args.get("command").and_then(|v| v.as_str()).unwrap_or("?");
            format!("{} {}", tool_name.cyan(), cmd.yellow())
        },
        "glob" => {
            let pattern = args.get("pattern").and_then(|v| v.as_str()).unwrap_or("?");
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
            format!("{} {} in {}", tool_name.cyan(), pattern.yellow(), path.yellow())
        },
        "grep" => {
            let pattern = args.get("pattern").and_then(|v| v.as_str()).unwrap_or("?");
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
            format!("{} {} in {}", tool_name.cyan(), pattern.yellow(), path.yellow())
        },
        "ls" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
            format!("{} {}", tool_name.cyan(), path.yellow())
        },
        "apply_patch" | "multiedit" | "edit" => {
            let path = args.get("file_path").and_then(|v| v.as_str()).unwrap_or("?");
            format!("{} {}", tool_name.cyan(), path.yellow())
        },
        "question" => {
            let question = args.get("question").and_then(|v| v.as_str()).unwrap_or("?");
            format!("{} {}", tool_name.cyan(), question.yellow())
        },
        "task" => {
            let desc = args.get("description").and_then(|v| v.as_str()).unwrap_or("?");
            format!("{} {}", tool_name.cyan(), desc.yellow())
        },
        "web_fetch" => {
            let url = args.get("url").and_then(|v| v.as_str()).unwrap_or("?");
            format!("{} {}", tool_name.cyan(), url.yellow())
        },
        "web_search" | "code_search" => {
            let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("?");
            format!("{} {}", tool_name.cyan(), query.yellow())
        },
        "lsp" => {
            let operation = args.get("operation").and_then(|v| v.as_str()).unwrap_or("?");
            let path = args.get("filePath").and_then(|v| v.as_str()).unwrap_or("?");
            format!("{} {} on {}", tool_name.cyan(), operation.yellow(), path.yellow())
        },
        "batch" => {
            format!("{} multiple tools", tool_name.cyan())
        },
        _ => {
            let args_str = serde_json::to_string_pretty(args).unwrap_or_default();
            format!("{} with arguments:\n{}", tool_name.cyan(), args_str.blue())
        },
    }
}

pub async fn run_agent_turn(
    agent: &Agent<deepseek::CompletionModel>,
    input: &str,
    chat_history: &mut Vec<Message>,
) -> Result<()> {
    let mut stream = agent.stream_prompt(input).with_history(chat_history.clone()).await;

    print!("{}: ", "Aries".green().bold());
    let mut full_response = String::new();

    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(MultiTurnStreamItem::StreamAssistantItem(StreamedAssistantContent::Text(Text { text }))) => {
                print!("{}", text);
                std::io::stdout().flush().unwrap_or_default();
                full_response.push_str(&text);
            },
            Ok(MultiTurnStreamItem::StreamAssistantItem(StreamedAssistantContent::ToolCall { tool_call, .. })) => {
                let formatted_tool = format_tool_args(&tool_call.function.name, &tool_call.function.arguments);
                println!("\n{}: Using tool {}", "Aries".green().bold(), formatted_tool);
            },
            Ok(MultiTurnStreamItem::FinalResponse(res)) => {
                if let Some(history) = res.history() {
                    *chat_history = history.to_vec();
                }
            },
            Err(e) => eprintln!("\n{}: {}", "Error streaming chunk".red(), e),
            _ => {},
        }
    }
    println!();
    Ok(())
}
