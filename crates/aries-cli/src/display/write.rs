use aries_core::tools::write::{NAME, WriteArgs, WriteOutput};

use crate::display::preview;
use crate::theme::Theme;

pub fn format_tool_call(args: &str, theme: &Theme) -> (String, Option<String>) {
    let args = serde_json::from_str::<WriteArgs>(args);

    let (first, rest) = match args {
        Ok(args) => {
            let first = args.file_path.display().to_string();
            let rest = Some(theme.dimmed(&preview(args.content)).to_string());
            (first, rest)
        },
        Err(_) => return (String::from("?"), None),
    };

    (format!("{} {}", theme.cyan_text(NAME), theme.yellow_text(&first)), rest)
}

pub fn format_tool_result(result: &str, theme: Theme) -> String {
    let output = serde_json::from_str::<WriteOutput>(result);

    match output {
        Ok(output) => {
            let text = output.to_string();
            if output.success {
                theme.green_text(&text).to_string()
            } else {
                theme.red_text(&text).to_string()
            }
        },
        Err(_) => theme.red_text(result).to_string(),
    }
}
