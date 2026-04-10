pub mod apply_patch;
pub mod batch;
pub mod code_search;
pub mod common;
pub mod edit;
pub mod glob;
pub mod grep;
pub mod ls;
pub mod lsp;
pub mod multi_edit;
pub mod question;
pub mod read_file;
pub mod shell_command;
pub mod task;
pub mod web_fetch;
pub mod web_search;
pub mod write_file;

use aries_core::tools::{
    ApplyPatchTool, BatchTool, CodeSearchTool, EditTool, GlobTool, GrepTool, LsTool, LspTool, MultiEditTool,
    QuestionTool, ReadFileTool, ShellCommand, TaskTool, WebFetchTool, WebSearchTool, WriteFileTool,
};
use aries_theme::Theme;
use itertools::Itertools;
use rig::providers::openai;
use rig::tool::Tool;

pub fn format_tool_call_args(tool_name: &str, args: &str, theme: &Theme) -> String {
    match tool_name {
        ReadFileTool::NAME => read_file::format_call(args, theme),
        WriteFileTool::NAME => write_file::format_call(args, theme),
        ShellCommand::NAME => shell_command::format_call(args, theme),
        GlobTool::NAME => glob::format_call(args, theme),
        GrepTool::NAME => grep::format_call(args, theme),
        LsTool::NAME => ls::format_call(args, theme),
        ApplyPatchTool::NAME => apply_patch::format_call(args, theme),
        EditTool::NAME => edit::format_call(args, theme),
        MultiEditTool::NAME => multi_edit::format_call(args, theme),
        BatchTool::<openai::CompletionModel, ()>::NAME => batch::format_call(args, theme),
        QuestionTool::NAME => question::format_call(args, theme),
        TaskTool::<openai::CompletionModel, ()>::NAME => task::format_call(args, theme),
        WebFetchTool::NAME => web_fetch::format_call(args, theme),
        WebSearchTool::NAME => web_search::format_call(args, theme),
        CodeSearchTool::NAME => code_search::format_call(args, theme),
        LspTool::NAME => lsp::format_call(args, theme),
        _ => format_unknown_call(tool_name, args, theme),
    }
}

pub fn format_tool_result_output(tool_name: &str, raw_text: &str) -> String {
    let output = match tool_name {
        ReadFileTool::NAME => read_file::format_result(raw_text),
        WriteFileTool::NAME => write_file::format_result(raw_text),
        ShellCommand::NAME => shell_command::format_result(raw_text),
        GlobTool::NAME => glob::format_result(raw_text),
        GrepTool::NAME => grep::format_result(raw_text),
        LsTool::NAME => ls::format_result(raw_text),
        ApplyPatchTool::NAME => apply_patch::format_result(raw_text),
        EditTool::NAME => edit::format_result(raw_text),
        MultiEditTool::NAME => multi_edit::format_result(raw_text),
        BatchTool::<openai::CompletionModel, ()>::NAME => batch::format_result(raw_text),
        QuestionTool::NAME => question::format_result(raw_text),
        TaskTool::<openai::CompletionModel, ()>::NAME => task::format_result(raw_text),
        WebFetchTool::NAME => web_fetch::format_result(raw_text),
        WebSearchTool::NAME => web_search::format_result(raw_text),
        CodeSearchTool::NAME => code_search::format_result(raw_text),
        LspTool::NAME => lsp::format_result(raw_text),
        _ => raw_text.to_string(),
    };

    if output.is_empty() { "No output".to_string() } else { output }
}

pub fn display_token_usage(usage: &rig::completion::Usage, theme: &Theme) {
    println!(
        "\n{} total={} input={} (cached={}) output={}\n",
        theme.dimmed("Token usage:"),
        theme.dimmed(&usage.total_tokens.to_string()),
        theme.dimmed(&usage.input_tokens.to_string()),
        theme.dimmed(&usage.cached_input_tokens.to_string()),
        theme.dimmed(&usage.output_tokens.to_string())
    );
}

pub fn format_unknown_call(tool_name: &str, args: &str, theme: &Theme) -> String {
    let args_str = serde_json::from_str::<serde_json::Value>(args)
        .and_then(|value| serde_json::to_string_pretty(&value))
        .unwrap_or_else(|_| args.to_string());
    format!("{} with arguments:\n{}", theme.cyan_text(tool_name), theme.blue_text(&args_str))
}

const MAX_LINES: usize = 5;

pub fn preview(content: &str) -> String {
    let lines: Vec<_> = content.lines().map(|line| format!("| {line}")).collect();
    let len = lines.len();

    if len > MAX_LINES {
        let preview = lines[..MAX_LINES].iter().join("\n");
        format!("{}\n+ ... ({} more lines truncated)", preview, len - MAX_LINES)
    } else {
        content.to_string()
    }
}
