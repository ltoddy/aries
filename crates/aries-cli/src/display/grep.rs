use aries_core::tools::grep::{GrepArgs, GrepOutput, NAME};

use crate::display::preview;
use crate::theme::Theme;

pub fn format_tool_call(args: &str, theme: &Theme) -> (String, Option<String>) {
    let args = serde_json::from_str::<GrepArgs>(args);

    let first = match args {
        Ok(args) => {
            let mut content = args.pattern;
            if let Some(include) = args.include {
                content.push_str(&format!(", include = {include}"));
            }
            content
        },
        Err(_) => return (String::from("?"), None),
    };

    (format!("{} {}", theme.cyan_text(NAME), theme.yellow_text(&first)), None)
}

pub fn format_tool_result(result: &str, theme: Theme) -> String {
    let output = serde_json::from_str::<GrepOutput>(result);

    match output {
        Ok(output) => theme.dimmed(&preview(output.to_string())).to_string(),
        Err(_) => theme.red_text(result).to_string(),
    }
}
