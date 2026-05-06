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
pub mod skill;
pub mod task;
pub mod web_fetch;
pub mod web_search;
pub mod write_file;

use aries_core::tools;
use itertools::Itertools;

use crate::theme::Theme;

pub fn format_tool_call_args(
    tool_name: &str,
    args: &str,
    theme: &Theme,
) -> (String, Option<String>) {
    match tool_name {
        tools::read::NAME => read_file::format_tool_call(args, theme),
        tools::write::NAME => write_file::format_tool_call(args, theme),
        tools::bash::NAME => shell_command::format_tool_call(args, theme),
        tools::glob::NAME => glob::format_tool_call(args, theme),
        tools::grep::NAME => grep::format_tool_call(args, theme),
        tools::ls::NAME => ls::format_tool_call(args, theme),
        tools::apply_patch::NAME => apply_patch::format_tool_call(args, theme),
        tools::edit::NAME => edit::format_tool_call(args, theme),
        tools::multiedit::NAME => multi_edit::format_tool_call(args, theme),
        tools::batch::NAME => batch::format_tool_call(args, theme),
        tools::question::NAME => question::format_tool_call(args, theme),
        tools::task::NAME => task::format_tool_call(args, theme),
        tools::webfetch::NAME => web_fetch::format_tool_call(args, theme),
        tools::websearch::NAME => web_search::format_tool_call(args, theme),
        tools::codesearch::NAME => code_search::format_tool_call(args, theme),
        tools::lsp::NAME => lsp::format_tool_call(args, theme),
        tools::skill::NAME => skill::format_tool_call(args, theme),
        _ => format_unknown_call(tool_name, args, theme),
    }
}

pub fn format_tool_result_output(tool_name: &str, result: &str, theme: Theme) -> String {
    let output = match tool_name {
        tools::read::NAME => read_file::format_tool_result(result, theme),
        tools::write::NAME => write_file::format_tool_result(result, theme),
        tools::bash::NAME => shell_command::format_tool_result(result, theme),
        tools::glob::NAME => glob::format_tool_result(result, theme),
        tools::grep::NAME => grep::format_tool_result(result, theme),
        tools::ls::NAME => ls::format_tool_result(result, theme),
        tools::apply_patch::NAME => apply_patch::format_tool_result(result, theme),
        tools::edit::NAME => edit::format_tool_result(result, theme),
        tools::multiedit::NAME => multi_edit::format_tool_result(result, theme),
        tools::batch::NAME => batch::format_tool_result(result, theme),
        tools::question::NAME => question::format_tool_result(result, theme),
        tools::task::NAME => task::format_tool_result(result, theme),
        tools::webfetch::NAME => web_fetch::format_tool_result(result, theme),
        tools::websearch::NAME => web_search::format_tool_result(result, theme),
        tools::codesearch::NAME => code_search::format_tool_result(result, theme),
        tools::lsp::NAME => lsp::format_tool_result(result, theme),
        tools::skill::NAME => skill::format_tool_result(result, theme),
        _ => result.to_string(),
    };

    if output.is_empty() { "No output".to_string() } else { output }
}

pub fn display_token_usage(usage: &rig::completion::Usage, theme: &Theme) {
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

pub fn preview(content: impl AsRef<str>) -> String {
    const MAX_LINES: usize = 5;

    let content = content.as_ref();
    let lines: Vec<_> = content.lines().map(|line| format!("| {line}")).collect();
    let len = lines.len();

    if len > MAX_LINES {
        let preview = lines[..MAX_LINES].iter().join("\n");
        format!("{}\n+ ... ({} more lines truncated)", preview, len - MAX_LINES)
    } else {
        lines.join("\n")
    }
}
