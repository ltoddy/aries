use aries_core::tools::{ShellCommand, ShellCommandArgs, ShellCommandOutput};
use aries_theme::Theme;
use rig::tool::Tool;

use crate::display::preview;

pub fn format_tool_call(args: &str, theme: &Theme) -> (String, Option<String>) {
    const NAME: &str = ShellCommand::NAME;
    let args = serde_json::from_str::<ShellCommandArgs>(args);

    let first = match args {
        Ok(args) => args.command,
        Err(_) => return (String::from("?"), None),
    };

    (format!("{} {}", theme.cyan_text(NAME), theme.yellow_text(&first)), None)
}

pub fn format_tool_result(raw_text: &str, theme: Theme) -> String {
    serde_json::from_str::<ShellCommandOutput>(raw_text)
        .map(|output| {
            let mut text = String::new();
            if !output.stdout.is_empty() {
                text.push_str(&output.stdout);
            }
            if !output.stderr.is_empty() {
                if !text.is_empty() {
                    text.push('\n');
                }
                text.push_str(&output.stderr);
            }
            theme.dimmed(&preview(&text)).to_string()
        })
        .unwrap_or_else(|_| theme.dimmed(&preview(raw_text)).to_string())
}
