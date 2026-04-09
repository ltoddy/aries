use aries_theme::Theme;
use serde_json::Value;

pub fn format_unknown_call(tool_name: &str, args: &Value, theme: &Theme) -> String {
    let args_str = serde_json::to_string_pretty(args).unwrap_or_else(|_| String::new());
    format!("{} with arguments:\n{}", theme.cyan_text(tool_name), theme.blue_text(&args_str))
}
