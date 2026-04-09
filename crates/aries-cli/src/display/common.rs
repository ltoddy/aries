use aries_theme::Theme;

pub fn format_unknown_call(tool_name: &str, args: &str, theme: &Theme) -> String {
    let args_str = serde_json::from_str::<serde_json::Value>(args)
        .and_then(|value| serde_json::to_string_pretty(&value))
        .unwrap_or_else(|_| args.to_string());
    format!("{} with arguments:\n{}", theme.cyan_text(tool_name), theme.blue_text(&args_str))
}
