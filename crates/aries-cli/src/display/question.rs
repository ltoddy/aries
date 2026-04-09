use aries_core::tools::{QuestionArgs, QuestionOutput, QuestionTool};
use aries_theme::Theme;
use rig::tool::Tool;

pub fn format_call(args: &str, theme: &Theme) -> String {
    const NAME: &str = QuestionTool::NAME;
    let args = serde_json::from_str::<QuestionArgs>(args);

    let args = match args {
        Ok(args) => args.question,
        Err(_) => String::from("?"),
    };

    format!("{} {}", theme.cyan_text(NAME), theme.yellow_text(&args))
}

pub fn format_result(raw_text: &str) -> String {
    serde_json::from_str::<QuestionOutput>(raw_text)
        .map(|output| output.answers.join("\n"))
        .unwrap_or_else(|_| raw_text.to_string())
}
