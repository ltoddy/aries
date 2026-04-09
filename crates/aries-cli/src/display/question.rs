use aries_core::tools::{QuestionOutput, QuestionTool};
use aries_theme::Theme;
use rig::tool::Tool;
use serde_json::Value;

pub fn format_call(args: &Value, theme: &Theme) -> String {
    let question = args.get("question").and_then(|v| v.as_str()).unwrap_or("?");
    format!("{} {}", theme.cyan_text(QuestionTool::NAME), theme.yellow_text(question))
}

pub fn format_result(raw_text: &str) -> String {
    serde_json::from_str::<QuestionOutput>(raw_text)
        .map(|output| output.answers.join("\n"))
        .unwrap_or_else(|_| raw_text.to_string())
}
