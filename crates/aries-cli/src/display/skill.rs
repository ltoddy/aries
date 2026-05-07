use aries_core::tools::skill::NAME;
use aries_core::tools::{SkillArgs, SkillOutput};

use crate::display::preview;
use crate::theme::Theme;

pub fn format_tool_call(args: &str, theme: &Theme) -> (String, Option<String>) {
    let args = serde_json::from_str::<SkillArgs>(args);

    let first = match args {
        Ok(args) => args.name,
        Err(_) => return (String::from("?"), None),
    };

    (format!("{}: {}", theme.cyan_text(NAME), theme.yellow_text(&first)), None)
}

pub fn format_tool_result(result: &str, theme: Theme) -> String {
    let output = serde_json::from_str::<SkillOutput>(result);

    match output {
        Ok(output) => theme.dimmed(&preview(output.to_string())).to_string(),
        Err(_) => theme.red_text(result).to_string(),
    }
}
