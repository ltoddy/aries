use aries_core::tools::{ShellCommand, ShellCommandArgs, ShellCommandOutput};
use aries_theme::Theme;
use rig::tool::Tool;

use crate::display::preview;

pub fn format_call(args: &str, theme: &Theme) -> String {
    const NAME: &str = ShellCommand::NAME;
    let args = serde_json::from_str::<ShellCommandArgs>(args);

    let args = match args {
        Ok(args) => args.command,
        Err(_) => String::from("?"),
    };

    format!("{} {}", theme.cyan_text(NAME), theme.yellow_text(&args))
}

pub fn format_result(raw_text: &str) -> String {
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
            preview(&text)
        })
        .unwrap_or_else(|_| preview(raw_text))
}
