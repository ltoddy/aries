use std::path::PathBuf;

use aries_core::tools::{LsArgs, LsOutput, LsTool};
use aries_theme::Theme;
use rig::tool::Tool;

use crate::display::preview;

pub fn format_tool_call(args: &str, theme: &Theme) -> (String, Option<String>) {
    let args = serde_json::from_str::<LsArgs>(args);

    let first = match args {
        Ok(args) => args.path.unwrap_or_else(|| PathBuf::from(".")).display().to_string(),
        Err(_) => return (String::from("?"), None),
    };

    (format!("{} {}", theme.cyan_text(LsTool::NAME), theme.yellow_text(&first)), None)
}

pub fn format_tool_result(result: &str, theme: Theme) -> String {
    let output = serde_json::from_str::<LsOutput>(result);

    match output {
        Ok(output) => theme.dimmed(&preview(&output.entries.join("\n"))).to_string(),
        Err(err) => theme.red_text(&format!("Error as follow: {err}")).to_string(),
    }
}
