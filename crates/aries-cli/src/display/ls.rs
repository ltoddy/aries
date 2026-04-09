use aries_core::tools::{LsOutput, LsTool};
use aries_theme::Theme;
use rig::tool::Tool;
use serde_json::Value;

pub fn format_call(args: &Value, theme: &Theme) -> String {
    let path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
    format!("{} {}", theme.cyan_text(LsTool::NAME), theme.yellow_text(path))
}

pub fn format_result(raw_text: &str) -> String {
    serde_json::from_str::<LsOutput>(raw_text)
        .map(|output| output.entries.join("\n"))
        .unwrap_or_else(|_| raw_text.to_string())
}
