use aries_core::tools::{ShellCommand, ShellCommandOutput};
use aries_theme::Theme;
use rig::tool::Tool;
use serde_json::Value;

pub fn format_call(args: &Value, theme: &Theme) -> String {
    let command = args.get("command").and_then(|v| v.as_str()).unwrap_or("?");
    format!("{} {}", theme.cyan_text(ShellCommand::NAME), theme.yellow_text(command))
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
            text
        })
        .unwrap_or_else(|_| raw_text.to_string())
}
