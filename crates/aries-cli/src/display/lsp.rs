use aries_core::tools::{LspOutput, LspTool};
use aries_theme::Theme;
use rig::tool::Tool;
use serde_json::Value;

pub fn format_call(args: &Value, theme: &Theme) -> String {
    let operation = args.get("operation").and_then(|v| v.as_str()).unwrap_or("?");
    let path = args.get("filePath").and_then(|v| v.as_str()).unwrap_or("?");
    format!("{} {} on {}", theme.cyan_text(LspTool::NAME), theme.yellow_text(operation), theme.yellow_text(path))
}

pub fn format_result(raw_text: &str) -> String {
    serde_json::from_str::<LspOutput>(raw_text)
        .map(|output| {
            if output.result.is_null() {
                "LSP operation successful".to_string()
            } else if let Some(s) = output.result.as_str() {
                s.to_string()
            } else {
                format!("LSP result: {}", output.result)
            }
        })
        .unwrap_or_else(|_| raw_text.to_string())
}
