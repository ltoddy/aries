use colored::Colorize;
use rig::tool::Tool;
use serde_json::Value;

use crate::theme::Theme;
use crate::tools::{
    ApplyPatchOutput, ApplyPatchTool, CodeSearchOutput, CodeSearchTool, EditOutput, EditTool, GlobOutput, GlobTool,
    GrepOutput, GrepTool, LsOutput, LsTool, LspOutput, LspTool, MultiEditOutput, MultiEditTool, QuestionOutput,
    QuestionTool, ReadFileOutput, ReadFileTool, ShellCommand, ShellCommandOutput, TaskOutput, TaskTool, WebFetchOutput,
    WebFetchTool, WebSearchOutput, WebSearchTool, WriteFileOutput, WriteFileTool,
};

pub fn extract_tool_result_raw_text(json_str: &str) -> String {
    let mut raw_text = String::new();
    if let Ok(obj) = serde_json::from_str::<Value>(json_str) {
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
            raw_text.push_str(json_str);
        }
    } else {
        raw_text.push_str("Tool execution completed.");
    }
    raw_text
}

pub fn format_tool_call_args(tool_name: &str, args: &Value, theme: &Theme) -> String {
    match tool_name {
        ReadFileTool::NAME => {
            let path = args.get("file_path").and_then(|v| v.as_str()).unwrap_or("?");
            format!("{} {}", theme.cyan_text(tool_name), theme.yellow_text(path))
        },
        WriteFileTool::NAME => {
            let path = args.get("file_path").and_then(|v| v.as_str()).unwrap_or("?");
            format!("{} {}", theme.cyan_text(tool_name), theme.yellow_text(path))
        },
        ShellCommand::NAME => {
            let cmd = args.get("command").and_then(|v| v.as_str()).unwrap_or("?");
            format!("{} {}", theme.cyan_text(tool_name), theme.yellow_text(cmd))
        },
        GlobTool::NAME => {
            let pattern = args.get("pattern").and_then(|v| v.as_str()).unwrap_or("?");
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
            format!("{} {} in {}", theme.cyan_text(tool_name), theme.yellow_text(pattern), theme.yellow_text(path))
        },
        GrepTool::NAME => {
            let pattern = args.get("pattern").and_then(|v| v.as_str()).unwrap_or("?");
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
            format!("{} {} in {}", theme.cyan_text(tool_name), theme.yellow_text(pattern), theme.yellow_text(path))
        },
        LsTool::NAME => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
            format!("{} {}", theme.cyan_text(tool_name), theme.yellow_text(path))
        },
        ApplyPatchTool::NAME | MultiEditTool::NAME | EditTool::NAME => {
            let path = args.get("file_path").and_then(|v| v.as_str()).unwrap_or("?");
            format!("{} {}", theme.cyan_text(tool_name), theme.yellow_text(path))
        },
        QuestionTool::NAME => {
            let question = args.get("question").and_then(|v| v.as_str()).unwrap_or("?");
            format!("{} {}", theme.cyan_text(tool_name), theme.yellow_text(question))
        },
        TaskTool::NAME => {
            let desc = args.get("description").and_then(|v| v.as_str()).unwrap_or("?");
            format!("{} {}", theme.cyan_text(tool_name), theme.yellow_text(desc))
        },
        WebFetchTool::NAME => {
            let url = args.get("url").and_then(|v| v.as_str()).unwrap_or("?");
            format!("{} {}", theme.cyan_text(tool_name), theme.yellow_text(url))
        },
        WebSearchTool::NAME | CodeSearchTool::NAME => {
            let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("?");
            format!("{} {}", theme.cyan_text(tool_name), theme.yellow_text(query))
        },
        LspTool::NAME => {
            let operation = args.get("operation").and_then(|v| v.as_str()).unwrap_or("?");
            let path = args.get("filePath").and_then(|v| v.as_str()).unwrap_or("?");
            format!("{} {} on {}", theme.cyan_text(tool_name), theme.yellow_text(operation), theme.yellow_text(path))
        },
        _ => {
            let args_str = serde_json::to_string_pretty(args).unwrap_or_else(|_| String::new());
            format!("{} with arguments:\n{}", theme.cyan_text(tool_name), theme.blue_text(&args_str))
        },
    }
}

pub fn format_tool_result_output(tool_name: &str, raw_text: &str) -> String {
    let mut output_str = String::new();

    match tool_name {
        ReadFileTool::NAME => {
            if let Ok(output) = serde_json::from_str::<ReadFileOutput>(raw_text) {
                output_str = output.content;
            } else {
                output_str = raw_text.to_string();
            }
        },
        WriteFileTool::NAME => {
            if let Ok(output) = serde_json::from_str::<WriteFileOutput>(raw_text) {
                output_str = if output.success {
                    "File written successfully".to_string()
                } else {
                    "Failed to write file".to_string()
                };
            } else {
                output_str = raw_text.to_string();
            }
        },
        ShellCommand::NAME => {
            if let Ok(output) = serde_json::from_str::<ShellCommandOutput>(raw_text) {
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
                output_str = raw_text.to_string();
            }
        },
        GlobTool::NAME => {
            if let Ok(output) = serde_json::from_str::<GlobOutput>(raw_text) {
                output_str = output.files.join("\n");
            } else {
                output_str = raw_text.to_string();
            }
        },
        GrepTool::NAME => {
            if let Ok(output) = serde_json::from_str::<GrepOutput>(raw_text) {
                output_str = output.matches.join("\n");
            } else {
                output_str = raw_text.to_string();
            }
        },
        LsTool::NAME => {
            if let Ok(output) = serde_json::from_str::<LsOutput>(raw_text) {
                output_str = output.entries.join("\n");
            } else {
                output_str = raw_text.to_string();
            }
        },
        ApplyPatchTool::NAME => {
            if let Ok(output) = serde_json::from_str::<ApplyPatchOutput>(raw_text) {
                output_str = output.message;
            } else {
                output_str = raw_text.to_string();
            }
        },
        EditTool::NAME => {
            if let Ok(output) = serde_json::from_str::<EditOutput>(raw_text) {
                output_str = output.message;
            } else {
                output_str = raw_text.to_string();
            }
        },
        MultiEditTool::NAME => {
            if let Ok(output) = serde_json::from_str::<MultiEditOutput>(raw_text) {
                output_str = output.message;
            } else {
                output_str = raw_text.to_string();
            }
        },
        QuestionTool::NAME => {
            if let Ok(output) = serde_json::from_str::<QuestionOutput>(raw_text) {
                output_str = output.answers.join("\n");
            } else {
                output_str = raw_text.to_string();
            }
        },
        TaskTool::NAME => {
            if let Ok(output) = serde_json::from_str::<TaskOutput>(raw_text) {
                output_str = output.result;
            } else {
                output_str = raw_text.to_string();
            }
        },
        WebFetchTool::NAME => {
            if let Ok(output) = serde_json::from_str::<WebFetchOutput>(raw_text) {
                output_str = output.content;
            } else {
                output_str = raw_text.to_string();
            }
        },
        WebSearchTool::NAME => {
            if let Ok(output) = serde_json::from_str::<WebSearchOutput>(raw_text) {
                output_str = output.results;
            } else {
                output_str = raw_text.to_string();
            }
        },
        CodeSearchTool::NAME => {
            if let Ok(output) = serde_json::from_str::<CodeSearchOutput>(raw_text) {
                output_str = output.results;
            } else {
                output_str = raw_text.to_string();
            }
        },
        LspTool::NAME => {
            if let Ok(output) = serde_json::from_str::<LspOutput>(raw_text) {
                output_str = if output.result.is_null() {
                    "LSP operation successful".to_string()
                } else if let Some(s) = output.result.as_str() {
                    s.to_string()
                } else {
                    format!("LSP result: {}", output.result)
                };
            } else {
                output_str = raw_text.to_string();
            }
        },
        _ => {
            output_str = raw_text.to_string();
        },
    }

    if output_str.is_empty() {
        output_str = "No output".to_string();
    }

    output_str
}

pub fn display_tool_call(tool_name: &str, args: &Value, theme: &Theme) {
    let formatted_tool = format_tool_call_args(tool_name, args, theme);
    println!("\n{} {}", theme.cyan_text("•").bold(), formatted_tool);
}

pub fn display_tool_result(tool_name: &str, json_str: &str, theme: &Theme) {
    let raw_text = extract_tool_result_raw_text(json_str);
    let output_str = format_tool_result_output(tool_name, &raw_text);

    let max_lines = 7;
    let lines: Vec<&str> = output_str.lines().collect();

    if lines.len() > max_lines {
        for line in lines.iter().take(max_lines) {
            println!("  {}", theme.dimmed(line));
        }
        println!("  ... ({} more lines truncated)", lines.len() - max_lines);
    } else {
        for line in lines {
            println!("  {}", theme.dimmed(line));
        }
    }
}

pub fn display_token_usage(usage: &rig::completion::Usage, theme: &Theme) {
    println!(
        "\n{} total={} input={} (cached={}) output={}",
        theme.dimmed("Token usage:"),
        theme.dimmed(&usage.total_tokens.to_string()),
        theme.dimmed(&usage.input_tokens.to_string()),
        theme.dimmed(&usage.cached_input_tokens.to_string()),
        theme.dimmed(&usage.output_tokens.to_string())
    );
}
