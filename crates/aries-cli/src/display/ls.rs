use std::path::PathBuf;

use aries_core::tools::{LsArgs, LsOutput, LsTool};
use aries_theme::Theme;
use rig::tool::Tool;

pub fn format_call(args: &str, theme: &Theme) -> String {
    let args = serde_json::from_str::<LsArgs>(args);

    let args = match args {
        Ok(args) => args.path.unwrap_or_else(|| PathBuf::from(".")).display().to_string(),
        Err(_) => String::from("?"),
    };

    format!("{} {}", theme.cyan_text(LsTool::NAME), theme.yellow_text(&args))
}

pub fn format_result(raw_text: &str) -> String {
    serde_json::from_str::<LsOutput>(raw_text)
        .map(|output| output.entries.join("\n"))
        .unwrap_or_else(|_| raw_text.to_string())
}
