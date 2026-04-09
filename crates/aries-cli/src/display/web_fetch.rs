use aries_core::tools::{WebFetchOutput, WebFetchTool};
use aries_theme::Theme;
use rig::tool::Tool;
use serde_json::Value;

pub fn format_call(args: &Value, theme: &Theme) -> String {
    let url = args.get("url").and_then(|v| v.as_str()).unwrap_or("?");
    format!("{} {}", theme.cyan_text(WebFetchTool::NAME), theme.yellow_text(url))
}

pub fn format_result(raw_text: &str) -> String {
    serde_json::from_str::<WebFetchOutput>(raw_text)
        .map(|output| output.content)
        .unwrap_or_else(|_| raw_text.to_string())
}
