use aries_core::tools::bash::NAME;
use aries_core::tools::{BashArgs, BashOutput};

use crate::display::preview;
use crate::theme::Theme;

pub fn format_tool_call(args: &str, theme: &Theme) -> (String, Option<String>) {
    let args = serde_json::from_str::<BashArgs>(args);

    let first = match args {
        Ok(args) => args.command,
        Err(_) => String::from("?"),
    };

    (format!("{} {}", theme.cyan_text(NAME), theme.yellow_text(&first)), None)
}

pub fn format_tool_result(result: &str, theme: Theme) -> String {
    let output = serde_json::from_str::<BashOutput>(result);

    match output {
        Ok(output) => theme.dimmed(&preview(output.to_string())).to_string(),
        Err(_) => theme.red_text(result).to_string(),
    }
}
