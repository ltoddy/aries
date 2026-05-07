use aries_core::tools::bash::{NAME, ShellCommandArgs, ShellCommandOutput};

use crate::display::preview;
use crate::theme::Theme;

pub fn format_tool_call(args: &str, theme: &Theme) -> (String, Option<String>) {
    let args = serde_json::from_str::<ShellCommandArgs>(args);

    let first = match args {
        Ok(args) => args.command,
        Err(_) => String::from("?"),
    };

    (format!("{} {}", theme.cyan_text(NAME), theme.yellow_text(&first)), None)
}

pub fn format_tool_result(result: &str, theme: Theme) -> String {
    let output = serde_json::from_str::<ShellCommandOutput>(result);

    match output {
        Ok(output) => {
            let mut out = String::new();
            if !output.stdout.is_empty() {
                out.push_str(&output.stdout);
            }
            if !output.stderr.is_empty() {
                if !out.is_empty() {
                    out.push('\n');
                }
                out.push_str(&output.stderr);
            }
            theme.dimmed(&preview(out)).to_string()
        },
        Err(_) => theme.red_text(result).to_string(),
    }
}
