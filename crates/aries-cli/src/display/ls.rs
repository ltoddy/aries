use std::path::PathBuf;

use aries_core::tools::{LsArgs, LsOutput, LsTool};
use aries_theme::Theme;
use rig::tool::Tool;

pub fn format_tool_call(args: &str, theme: &Theme) -> (String, Option<String>) {
    let args = serde_json::from_str::<LsArgs>(args);

    let first = match args {
        Ok(args) => args.path.unwrap_or_else(|| PathBuf::from(".")).display().to_string(),
        Err(_) => return (String::from("?"), None),
    };

    (format!("{} {}", theme.cyan_text(LsTool::NAME), theme.yellow_text(&first)), None)
}

pub fn format_tool_result(raw_text: &str, theme: Theme) -> String {
    serde_json::from_str::<LsOutput>(raw_text)
        .map(|output| theme.dimmed(&output.entries.join("\n")).to_string())
        .unwrap_or_else(|_| theme.dimmed(raw_text).to_string())
}
