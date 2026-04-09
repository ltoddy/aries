use aries_theme::Theme;
use serde_json::Value;

pub fn format_call(args: &Value, theme: &Theme) -> String {
    let tool_calls = args.get("calls").and_then(|v| v.as_array()).map(|v| v.len()).unwrap_or(0);
    format!("{} {} tool calls", theme.cyan_text("batch"), theme.yellow_text(&tool_calls.to_string()))
}

pub fn format_result(raw_text: &str) -> String {
    raw_text.to_string()
}
