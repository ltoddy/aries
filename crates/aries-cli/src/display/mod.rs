pub mod apply_patch;
pub mod batch;
pub mod code_search;
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

pub fn format_tool_call_args(tool_name: &str, args: &str, theme: &Theme) -> (String, Option<String>) {
    match tool_name {
        ReadFileTool::NAME => read_file::format_tool_call(args, theme),
        WriteFileTool::NAME => write_file::format_tool_call(args, theme),
        ShellCommand::NAME => shell_command::format_tool_call(args, theme),
        GlobTool::NAME => glob::format_tool_call(args, theme),
        GrepTool::NAME => grep::format_tool_call(args, theme),
        LsTool::NAME => ls::format_tool_call(args, theme),
        ApplyPatchTool::NAME => apply_patch::format_tool_call(args, theme),
        EditTool::NAME => edit::format_tool_call(args, theme),
        MultiEditTool::NAME => multi_edit::format_tool_call(args, theme),
        BatchTool::<openai::CompletionModel, ()>::NAME => batch::format_tool_call(args, theme),
        QuestionTool::NAME => question::format_tool_call(args, theme),
        TaskTool::<openai::CompletionModel, ()>::NAME => task::format_tool_call(args, theme),
        WebFetchTool::NAME => web_fetch::format_tool_call(args, theme),
        WebSearchTool::NAME => web_search::format_tool_call(args, theme),
        CodeSearchTool::NAME => code_search::format_tool_call(args, theme),
        LspTool::NAME => lsp::format_tool_call(args, theme),
        _ => format_unknown_call(tool_name, args, theme),
    }
}

pub fn format_tool_result_output(tool_name: &str, result: &str, theme: Theme) -> String {
    let output = match tool_name {
        ReadFileTool::NAME => read_file::format_tool_result(result, theme),
        WriteFileTool::NAME => write_file::format_tool_result(result, theme),
        ShellCommand::NAME => shell_command::format_tool_result(result, theme),
        GlobTool::NAME => glob::format_tool_result(result, theme),
        GrepTool::NAME => grep::format_tool_result(result, theme),
        LsTool::NAME => ls::format_tool_result(result, theme),
        ApplyPatchTool::NAME => apply_patch::format_tool_result(result, theme),
        EditTool::NAME => edit::format_tool_result(result, theme),
        MultiEditTool::NAME => multi_edit::format_tool_result(result, theme),
        BatchTool::<openai::CompletionModel, ()>::NAME => batch::format_tool_result(result, theme),
        QuestionTool::NAME => question::format_tool_result(result, theme),
        TaskTool::<openai::CompletionModel, ()>::NAME => task::format_tool_result(result, theme),
        WebFetchTool::NAME => web_fetch::format_tool_result(result, theme),
        WebSearchTool::NAME => web_search::format_tool_result(result, theme),
        CodeSearchTool::NAME => code_search::format_tool_result(result, theme),
        LspTool::NAME => lsp::format_tool_result(result, theme),
        _ => result.to_string(),
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

pub fn format_unknown_call(tool_name: &str, args: &str, theme: &Theme) -> (String, Option<String>) {
    let args_str = serde_json::from_str::<serde_json::Value>(args)
        .and_then(|value| serde_json::to_string_pretty(&value))
        .unwrap_or_else(|_| args.to_string());

    (format!("{} {}", theme.cyan_text(tool_name), theme.yellow_text("(unknown tool)")), Some(args_str))
}

pub fn preview(content: &str) -> String {
    const MAX_LINES: usize = 5;

    let lines: Vec<_> = content.lines().map(|line| format!("| {line}")).collect();
    let len = lines.len();

    if len > MAX_LINES {
        let preview = lines[..MAX_LINES].iter().join("\n");
        format!("{}\n+ ... ({} more lines truncated)", preview, len - MAX_LINES)
    } else {
        lines.join("\n")
    }
}
