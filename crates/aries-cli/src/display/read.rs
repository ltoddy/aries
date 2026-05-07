use aries_core::tools::read::{NAME, ReadArgs, ReadOutput};

use crate::display::preview;
use crate::theme::Theme;

pub fn format_tool_call(args: &str, theme: &Theme) -> (String, Option<String>) {
    let args = serde_json::from_str::<ReadArgs>(args);

    let first = match args {
        Ok(args) => {
            let mut path = format!("{}", args.file_path.display());
            if let Some(offset) = args.offset {
                path.push_str(&format!(", offset = {offset}"));
            }
            path
        },
        Err(_) => return (String::from("?"), None),
    };

    (format!("{}: {}", theme.cyan_text(NAME), theme.yellow_text(&first)), None)
}

pub fn format_tool_result(result: &str, theme: Theme) -> String {
    match serde_json::from_str::<ReadOutput>(result) {
        Ok(output) => theme.dimmed(&preview(output.to_string())).to_string(),
        Err(_) => theme.red_text(result).to_string(),
    }
}
