use aries_core::tools::{WebFetchArgs, WebFetchOutput, WebFetchTool};
use aries_theme::Theme;
use rig::tool::Tool;

use crate::display::preview;

pub fn format_tool_call(args: &str, theme: &Theme) -> (String, Option<String>) {
    const NAME: &str = WebFetchTool::NAME;
    let args = serde_json::from_str::<WebFetchArgs>(args);

    let first = match args {
        Ok(args) => args.url,
        Err(_) => return (String::from("?"), None),
    };

    (format!("{} {}", theme.cyan_text(NAME), theme.yellow_text(&first)), None)
}

pub fn format_tool_result(raw_text: &str, theme: Theme) -> String {
    serde_json::from_str::<WebFetchOutput>(raw_text)
        .map(|output| theme.dimmed(&preview(&output.content)).to_string())
        .unwrap_or_else(|_| theme.dimmed(&preview(raw_text)).to_string())
}
