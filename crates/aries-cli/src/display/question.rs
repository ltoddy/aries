use aries_core::tools::question::{AskUserQuestionArgs, AskUserQuestionOutput, NAME};

use crate::theme::Theme;

pub fn format_tool_call(args: &str, theme: &Theme) -> (String, Option<String>) {
    let args = serde_json::from_str::<AskUserQuestionArgs>(args);

    let first = match args {
        Ok(args) => args.question,
        Err(_) => return (String::from("?"), None),
    };

    (format!("{} {}", theme.cyan_text(NAME), theme.yellow_text(&first)), None)
}

pub fn format_tool_result(raw_text: &str, theme: Theme) -> String {
    match serde_json::from_str::<AskUserQuestionOutput>(raw_text) {
        Ok(output) => theme.dimmed(&output.to_string()).to_string(),
        Err(_) => theme.red_text(raw_text).to_string(),
    }
}
