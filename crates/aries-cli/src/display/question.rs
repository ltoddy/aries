use aries_core::tools::{QuestionArgs, QuestionOutput, QuestionTool};
use aries_theme::Theme;
use rig::tool::Tool;

pub fn format_tool_call(args: &str, theme: &Theme) -> (String, Option<String>) {
    const NAME: &str = QuestionTool::NAME;
    let args = serde_json::from_str::<QuestionArgs>(args);

    let first = match args {
        Ok(args) => args.question,
        Err(_) => return (String::from("?"), None),
    };

    (format!("{} {}", theme.cyan_text(NAME), theme.yellow_text(&first)), None)
}

pub fn format_tool_result(raw_text: &str, theme: Theme) -> String {
    serde_json::from_str::<QuestionOutput>(raw_text)
        .map(|output| theme.dimmed(&output.answers.join("\n")).to_string())
        .unwrap_or_else(|_| theme.dimmed(raw_text).to_string())
}
