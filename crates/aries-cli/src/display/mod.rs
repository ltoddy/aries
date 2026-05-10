use aries_core::tools;
use itertools::Itertools;

use crate::theme::Theme;

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
            | tools::apply_patch::NAME
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
