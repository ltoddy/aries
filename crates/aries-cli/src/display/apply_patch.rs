use aries_core::tools::apply_patch::{ApplyPatchArgs, ApplyPatchOutput, NAME};

use crate::theme::Theme;

pub fn format_tool_call(args: &str, theme: &Theme) -> (String, Option<String>) {
    let args = serde_json::from_str::<ApplyPatchArgs>(args);

    let args = match args {
        Ok(args) => args,
        Err(_) => return (String::from("?"), None),
    };

    let file = args
        .patch
        .lines()
        .find_map(|line| line.strip_prefix("+++ b/").or_else(|| line.strip_prefix("--- a/")))
        .map(ToString::to_string)
        .unwrap_or_else(|| "?".to_string());

    // Colorized unified diff output (git diff style-ish).
    const MAX_DIFF_LINES: usize = 30;
    let mut out_lines = Vec::new();
    let mut total = 0usize;

    for line in args.patch.lines() {
        total += 1;
        if out_lines.len() >= MAX_DIFF_LINES {
            break;
        }

        let rendered = if line.starts_with("diff --git ") {
            theme.cyan_text(line).to_string()
        } else if line.starts_with("index ")
            || line.starts_with("new file mode ")
            || line.starts_with("deleted file mode ")
            || line.starts_with("similarity index ")
            || line.starts_with("rename from ")
            || line.starts_with("rename to ")
            || line.starts_with("\\ No newline at end of file")
        {
            theme.dimmed(line).to_string()
        } else if line.starts_with("--- ") {
            theme.red_text(line).to_string()
        } else if line.starts_with("+++ ") {
            theme.green_text(line).to_string()
        } else if line.starts_with("@@") {
            theme.blue_text(line).to_string()
        } else if line.starts_with('+') {
            // Avoid re-coloring file header lines.
            theme.green_text(line).to_string()
        } else if line.starts_with('-') {
            // Avoid re-coloring file header lines.
            theme.red_text(line).to_string()
        } else {
            line.to_string()
        };

        out_lines.push(rendered);
    }

    if total > out_lines.len() {
        out_lines.push(
            theme
                .dimmed(&format!("... ({} more lines truncated)", total - out_lines.len()))
                .to_string(),
        );
    }

    (format!("{} {}", theme.cyan_text(NAME), theme.yellow_text(&file)), Some(out_lines.join("\n")))
}

pub fn format_tool_result(raw_text: &str, theme: Theme) -> String {
    match serde_json::from_str::<ApplyPatchOutput>(raw_text) {
        Ok(output) => theme.dimmed(&output.message).to_string(),
        Err(_) => theme.red_text(raw_text).to_string(),
    }
}
