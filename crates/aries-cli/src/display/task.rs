use aries_core::tools::{TaskArgs, TaskOutput, TaskTool};
use aries_theme::Theme;
use rig::tool::Tool;

pub fn format_call(args: &str, theme: &Theme) -> String {
    const NAME: &str = TaskTool::<rig::providers::openai::CompletionModel, ()>::NAME;

    let args = serde_json::from_str::<TaskArgs>(args);

    let args = match args {
        Ok(args) => {
            let mut description = args.description;
            description.push_str(&format!(", subagent_type = {}", args.subagent_type));
            description
        },
        Err(_) => String::from("?"),
    };

    format!("{} {}", theme.cyan_text(NAME), theme.yellow_text(&args))
}

pub fn format_result(raw_text: &str) -> String {
    serde_json::from_str::<TaskOutput>(raw_text).map(|output| output.result).unwrap_or_else(|_| raw_text.to_string())
}
