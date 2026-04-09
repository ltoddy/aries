use aries_core::tools::{TaskOutput, TaskTool};
use aries_theme::Theme;
use rig::providers::openai;
use rig::tool::Tool;
use serde_json::Value;

pub fn format_call(args: &Value, theme: &Theme) -> String {
    let desc = args.get("description").and_then(|v| v.as_str()).unwrap_or("?");
    let subagent_type = args.get("subagent_type").and_then(|v| v.as_str()).unwrap_or("unknown");
    let agent_name = format!("Subagent [{}]", subagent_type);
    format!("Starting {} task: {}", theme.cyan_text(&agent_name), theme.yellow_text(desc))
}

pub fn format_result(raw_text: &str) -> String {
    let _ = TaskTool::<openai::CompletionModel, ()>::NAME;
    serde_json::from_str::<TaskOutput>(raw_text).map(|output| output.result).unwrap_or_else(|_| raw_text.to_string())
}
