use aries_core::tools::websearch::{NAME, WebSearchArgs, WebSearchOutput};
use aries_theme::Theme;

use crate::display::preview;

pub fn format_tool_call(args: &str, theme: &Theme) -> (String, Option<String>) {
    let args = serde_json::from_str::<WebSearchArgs>(args);

    let first = match args {
        Ok(args) => args.query,
        Err(_) => return (String::from("?"), None),
    };

    (format!("{} {}", theme.cyan_text(NAME), theme.yellow_text(&first)), None)
}

pub fn format_tool_result(result: &str, theme: Theme) -> String {
    let output = serde_json::from_str::<WebSearchOutput>(result);

    match output {
        Ok(output) => theme.dimmed(&preview(output.results)).to_string(),
        Err(err) => theme.red_text(&format!("Error as follow: {err}")).to_string(),
    }
}
