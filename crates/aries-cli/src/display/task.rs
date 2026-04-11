use aries_core::tools::task::{NAME, TaskArgs, TaskOutput};
use aries_theme::Theme;

pub fn format_tool_call(args: &str, theme: &Theme) -> (String, Option<String>) {
    let args = serde_json::from_str::<TaskArgs>(args);

    let (first, rest) = match args {
        Ok(args) => {
            let mut description = args.description;
            description.push_str(&format!(", subagent_type = {}", args.subagent_type));
            if let Some(task_id) = &args.task_id {
                description.push_str(&format!(", task_id = {}", task_id));
            }
            (description, Some(args.prompt))
        },
        Err(_) => return (String::from("?"), None),
    };

    (format!("{} {}", theme.cyan_text(NAME), theme.yellow_text(&first)), rest)
}

pub fn format_tool_result(raw_text: &str, theme: Theme) -> String {
    serde_json::from_str::<TaskOutput>(raw_text)
        .map(|output| theme.dimmed(&output.result).to_string())
        .unwrap_or_else(|_| theme.dimmed(raw_text).to_string())
}
