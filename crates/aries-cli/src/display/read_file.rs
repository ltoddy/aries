use aries_core::tools::read::{NAME, ReadFileArgs, ReadFileOutput};
use aries_theme::Theme;

use crate::display::preview;

pub fn format_tool_call(args: &str, theme: &Theme) -> (String, Option<String>) {
    let args = serde_json::from_str::<ReadFileArgs>(args);

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
    let output = serde_json::from_str::<ReadFileOutput>(result);

    match output {
        Ok(output) => theme.dimmed(&preview(&output.content)).to_string(),
        Err(err) => theme.red_text(&format!("Error as follow: {err}")).to_string(),
    }
}
