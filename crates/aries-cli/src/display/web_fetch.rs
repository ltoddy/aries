use aries_core::tools::webfetch::{NAME, WebFetchArgs, WebFetchOutput};
use aries_theme::Theme;

use crate::display::preview;

pub fn format_tool_call(args: &str, theme: &Theme) -> (String, Option<String>) {
    let args = serde_json::from_str::<WebFetchArgs>(args);

    let first = match args {
        Ok(args) => args.url,
        Err(_) => return (String::from("?"), None),
    };

    (format!("{} {}", theme.cyan_text(NAME), theme.yellow_text(&first)), None)
}

pub fn format_tool_result(result: &str, theme: Theme) -> String {
    let output = serde_json::from_str::<WebFetchOutput>(result);

    match output {
        Ok(output) => theme.dimmed(&preview(&output.content)).to_string(),
        Err(err) => theme.red_text(&format!("Error as follow: {err}")).to_string(),
    }
}
