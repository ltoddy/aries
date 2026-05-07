use std::path::PathBuf;

use aries_core::tools::ls::{LsArgs, LsOutput, NAME};

use crate::display::preview;
use crate::theme::Theme;

pub fn format_tool_call(args: &str, theme: &Theme) -> (String, Option<String>) {
    let args = serde_json::from_str::<LsArgs>(args);

    let first = match args {
        Ok(args) => args.path.unwrap_or_else(|| PathBuf::from(".")).display().to_string(),
        Err(_) => return (String::from("?"), None),
    };

    (format!("{} {}", theme.cyan_text(NAME), theme.yellow_text(&first)), None)
}

pub fn format_tool_result(result: &str, theme: Theme) -> String {
    let output = serde_json::from_str::<LsOutput>(result);

    match output {
        Ok(output) => theme.dimmed(&preview(output.entries.join("\n"))).to_string(),
        Err(_) => theme.red_text(result).to_string(),
    }
}
