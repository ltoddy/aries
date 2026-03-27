use std::io::Write;

use anyhow::Result;
use colored::Colorize;
use futures::StreamExt;
use rig::agent::{Agent, MultiTurnStreamItem};
use rig::completion::Message;
use rig::message::Text;
use rig::providers::deepseek;
use rig::streaming::{StreamedAssistantContent, StreamedUserContent, StreamingPrompt};
use rig::tool::Tool;
use serde_json::Value;

use crate::tools::{
    ApplyPatchOutput, ApplyPatchTool, BatchOutput, BatchTool, CodeSearchOutput,
    CodeSearchTool, EditOutput, EditTool, GlobOutput, GlobTool, GrepOutput, GrepTool,
    LsOutput, LsTool, LspOutput, LspTool, MultiEditOutput, MultiEditTool,
    QuestionOutput, QuestionTool, ReadFileOutput, ReadFileTool, ShellCommand,
    ShellCommandOutput, TaskOutput, TaskTool, WebFetchOutput, WebFetchTool,
    WebSearchOutput, WebSearchTool, WriteFileOutput, WriteFileTool,
};

fn format_tool_args(tool_name: &str, args: &Value) -> String {
    match tool_name {
        ReadFileTool::NAME => {
            let path = args.get("file_path").and_then(|v| v.as_str()).unwrap_or("?");
            format!("{} {}", tool_name.cyan(), path.yellow())
        },
        WriteFileTool::NAME => {
            let path = args.get("file_path").and_then(|v| v.as_str()).unwrap_or("?");
            format!("{} {}", tool_name.cyan(), path.yellow())
        },
        ShellCommand::NAME => {
            let cmd = args.get("command").and_then(|v| v.as_str()).unwrap_or("?");
            format!("{} {}", tool_name.cyan(), cmd.yellow())
        },
        GlobTool::NAME => {
            let pattern = args.get("pattern").and_then(|v| v.as_str()).unwrap_or("?");
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
            format!("{} {} in {}", tool_name.cyan(), pattern.yellow(), path.yellow())
        },
        GrepTool::NAME => {
            let pattern = args.get("pattern").and_then(|v| v.as_str()).unwrap_or("?");
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
            format!("{} {} in {}", tool_name.cyan(), pattern.yellow(), path.yellow())
        },
        LsTool::NAME => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
            format!("{} {}", tool_name.cyan(), path.yellow())
        },
        ApplyPatchTool::NAME | MultiEditTool::NAME | EditTool::NAME => {
            let path = args.get("file_path").and_then(|v| v.as_str()).unwrap_or("?");
            format!("{} {}", tool_name.cyan(), path.yellow())
        },
        QuestionTool::NAME => {
            let question = args.get("question").and_then(|v| v.as_str()).unwrap_or("?");
            format!("{} {}", tool_name.cyan(), question.yellow())
        },
        TaskTool::NAME => {
            let desc = args.get("description").and_then(|v| v.as_str()).unwrap_or("?");
            format!("{} {}", tool_name.cyan(), desc.yellow())
        },
        WebFetchTool::NAME => {
            let url = args.get("url").and_then(|v| v.as_str()).unwrap_or("?");
            format!("{} {}", tool_name.cyan(), url.yellow())
        },
        WebSearchTool::NAME | CodeSearchTool::NAME => {
            let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("?");
            format!("{} {}", tool_name.cyan(), query.yellow())
        },
        LspTool::NAME => {
            let operation = args.get("operation").and_then(|v| v.as_str()).unwrap_or("?");
            let path = args.get("filePath").and_then(|v| v.as_str()).unwrap_or("?");
            format!("{} {} on {}", tool_name.cyan(), operation.yellow(), path.yellow())
        },
        BatchTool::NAME => {
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
    let mut active_tools: std::collections::HashMap<String, String> = std::collections::HashMap::new();

    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(MultiTurnStreamItem::StreamAssistantItem(StreamedAssistantContent::Text(Text { text }))) => {
                print!("{}", text);
                std::io::stdout().flush().unwrap_or_default();
                full_response.push_str(&text);
            },
            Ok(MultiTurnStreamItem::StreamAssistantItem(StreamedAssistantContent::ToolCall { tool_call, .. })) => {
                active_tools.insert(tool_call.id.clone(), tool_call.function.name.clone());
                let formatted_tool = format_tool_args(&tool_call.function.name, &tool_call.function.arguments);
                println!("\n{} {}", "•".cyan().bold(), formatted_tool);
            },
            Ok(MultiTurnStreamItem::StreamUserItem(StreamedUserContent::ToolResult { tool_result, .. })) => {
                let tool_name = active_tools.get(&tool_result.id).cloned().unwrap_or_default();
                let mut raw_text = String::new();
                let json_str = serde_json::to_string(&tool_result).unwrap_or_default();
                
                if let Ok(obj) = serde_json::from_str::<Value>(&json_str) {
                    let results_arr = obj.get("content").and_then(|v| v.as_array());
                    if let Some(arr) = results_arr {
                        for item in arr {
                            if let Some(text_val) = item.get("text") {
                                if let Some(s) = text_val.as_str() {
                                    raw_text.push_str(s);
                                } else {
                                    raw_text.push_str(&text_val.to_string());
                                }
                            } else if let Some(content) = item.get("content") {
                                raw_text.push_str(&content.to_string());
                            }
                        }
                    } else {
                        raw_text.push_str(&json_str);
                    }
                } else {
                    raw_text.push_str("Tool execution completed.");
                }

                let mut output_str = String::new();
                
                match tool_name.as_str() {
                    ReadFileTool::NAME => {
                        if let Ok(output) = serde_json::from_str::<ReadFileOutput>(&raw_text) {
                            output_str = output.content;
                        } else {
                            output_str = raw_text;
                        }
                    },
                    WriteFileTool::NAME => {
                        if let Ok(output) = serde_json::from_str::<WriteFileOutput>(&raw_text) {
                            output_str = if output.success { "File written successfully".to_string() } else { "Failed to write file".to_string() };
                        } else {
                            output_str = raw_text;
                        }
                    },
                    ShellCommand::NAME => {
                        if let Ok(output) = serde_json::from_str::<ShellCommandOutput>(&raw_text) {
                            if !output.stdout.is_empty() {
                                output_str.push_str(&output.stdout);
                            }
                            if !output.stderr.is_empty() {
                                if !output_str.is_empty() {
                                    output_str.push('\n');
                                }
                                output_str.push_str(&output.stderr);
                            }
                        } else {
                            output_str = raw_text;
                        }
                    },
                    GlobTool::NAME => {
                        if let Ok(output) = serde_json::from_str::<GlobOutput>(&raw_text) {
                            output_str = output.files.join("\n");
                        } else {
                            output_str = raw_text;
                        }
                    },
                    GrepTool::NAME => {
                        if let Ok(output) = serde_json::from_str::<GrepOutput>(&raw_text) {
                            output_str = output.matches.join("\n");
                        } else {
                            output_str = raw_text;
                        }
                    },
                    LsTool::NAME => {
                        if let Ok(output) = serde_json::from_str::<LsOutput>(&raw_text) {
                            output_str = output.entries.join("\n");
                        } else {
                            output_str = raw_text;
                        }
                    },
                    ApplyPatchTool::NAME => {
                        if let Ok(output) = serde_json::from_str::<ApplyPatchOutput>(&raw_text) {
                            output_str = output.message;
                        } else {
                            output_str = raw_text;
                        }
                    },
                    EditTool::NAME => {
                        if let Ok(output) = serde_json::from_str::<EditOutput>(&raw_text) {
                            output_str = output.message;
                        } else {
                            output_str = raw_text;
                        }
                    },
                    MultiEditTool::NAME => {
                        if let Ok(output) = serde_json::from_str::<MultiEditOutput>(&raw_text) {
                            output_str = output.message;
                        } else {
                            output_str = raw_text;
                        }
                    },
                    QuestionTool::NAME => {
                        if let Ok(output) = serde_json::from_str::<QuestionOutput>(&raw_text) {
                            output_str = output.answers.join("\n");
                        } else {
                            output_str = raw_text;
                        }
                    },
                    TaskTool::NAME => {
                        if let Ok(output) = serde_json::from_str::<TaskOutput>(&raw_text) {
                            output_str = output.result;
                        } else {
                            output_str = raw_text;
                        }
                    },
                    WebFetchTool::NAME => {
                        if let Ok(output) = serde_json::from_str::<WebFetchOutput>(&raw_text) {
                            output_str = output.content;
                        } else {
                            output_str = raw_text;
                        }
                    },
                    WebSearchTool::NAME => {
                        if let Ok(output) = serde_json::from_str::<WebSearchOutput>(&raw_text) {
                            output_str = output.results;
                        } else {
                            output_str = raw_text;
                        }
                    },
                    CodeSearchTool::NAME => {
                        if let Ok(output) = serde_json::from_str::<CodeSearchOutput>(&raw_text) {
                            output_str = output.results;
                        } else {
                            output_str = raw_text;
                        }
                    },
                    BatchTool::NAME => {
                        if let Ok(output) = serde_json::from_str::<BatchOutput>(&raw_text) {
                            output_str = format!("Executed {} tools in batch\n", output.results.len());
                            for (i, res) in output.results.iter().enumerate() {
                                output_str.push_str(&format!("  [Tool {}]:\n", i + 1));
                                
                                if let Some(success) = res.get("success").and_then(|v| v.as_bool()) {
                                    if success {
                                        if let Some(result_val) = res.get("result") {
                                            // Format based on inner result content
                                            if let Some(obj) = result_val.as_object() {
                                                if let (Some(stdout), Some(stderr)) = (obj.get("stdout"), obj.get("stderr")) {
                                                    if let Some(out_str) = stdout.as_str() {
                                                        if !out_str.is_empty() {
                                                            output_str.push_str(&format!("    stdout: {}\n", out_str.trim()));
                                                        }
                                                    }
                                                    if let Some(err_str) = stderr.as_str() {
                                                        if !err_str.is_empty() {
                                                            output_str.push_str(&format!("    stderr: {}\n", err_str.trim()));
                                                        }
                                                    }
                                                } else if let Some(content) = obj.get("content").and_then(|v| v.as_str()) {
                                                    let preview: String = content.lines().take(5).collect::<Vec<_>>().join("\n    ");
                                                    if content.lines().count() > 5 {
                                                        output_str.push_str(&format!("    {}\n    ...\n", preview));
                                                    } else {
                                                        output_str.push_str(&format!("    {}\n", preview));
                                                    }
                                                } else {
                                                    output_str.push_str(&format!("    Success\n"));
                                                }
                                            } else {
                                                output_str.push_str(&format!("    {}\n", result_val));
                                            }
                                        }
                                    } else if let Some(err_val) = res.get("error") {
                                        output_str.push_str(&format!("    Error: {}\n", err_val));
                                    }
                                } else {
                                    output_str.push_str(&format!("    {}\n", res));
                                }
                            }
                        } else {
                            output_str = raw_text;
                        }
                    },
                    LspTool::NAME => {
                        if let Ok(output) = serde_json::from_str::<LspOutput>(&raw_text) {
                            output_str = if output.result.is_null() {
                                "LSP operation successful".to_string()
                            } else if let Some(s) = output.result.as_str() {
                                s.to_string()
                            } else {
                                format!("LSP result: {}", output.result)
                            };
                        } else {
                            output_str = raw_text;
                        }
                    },
                    _ => {
                        output_str = raw_text;
                    }
                }

                if output_str.is_empty() {
                    output_str = "No output".to_string();
                }

                let max_lines = 7;
                let lines: Vec<&str> = output_str.lines().collect();

                if lines.len() > max_lines {
                    for line in lines.iter().take(max_lines) {
                        println!("  {}", line.dimmed());
                    }
                    println!(
                        "  ... ({} more lines truncated)",
                        lines.len() - max_lines
                    );
                } else {
                    for line in lines {
                        println!("  {}", line.dimmed());
                    }
                }
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
