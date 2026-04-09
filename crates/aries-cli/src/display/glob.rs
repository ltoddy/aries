use aries_core::tools::{GlobOutput, GlobTool};
use aries_theme::Theme;
use rig::tool::Tool;
use serde_json::Value;

pub fn format_call(args: &Value, theme: &Theme) -> String {
    let pattern = args.get("pattern").and_then(|v| v.as_str()).unwrap_or("?");
    let path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
    format!("{} {} in {}", theme.cyan_text(GlobTool::NAME), theme.yellow_text(pattern), theme.yellow_text(path))
}

pub fn format_result(raw_text: &str) -> String {
    serde_json::from_str::<GlobOutput>(raw_text)
        .map(|output| output.files.join("\n"))
        .unwrap_or_else(|_| raw_text.to_string())
}
