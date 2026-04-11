use aries_core::tools::write::{NAME, WriteFileArgs, WriteFileOutput};
use aries_theme::Theme;

use crate::display::preview;

pub fn format_tool_call(args: &str, theme: &Theme) -> (String, Option<String>) {
    let args = serde_json::from_str::<WriteFileArgs>(args);

    let (first, rest) = match args {
        Ok(args) => {
            let first = args.file_path.display().to_string();
            let rest = Some(theme.dimmed(&preview(&args.content)).to_string());
            (first, rest)
        },
        Err(_) => return (String::from("?"), None),
    };

    (format!("{} {}", theme.cyan_text(NAME), theme.yellow_text(&first)), rest)
}

pub fn format_tool_result(result: &str, theme: Theme) -> String {
    let output = serde_json::from_str::<WriteFileOutput>(result);

    match output {
        Ok(output) => {
            if output.success {
                theme.green_text("File written successfully").to_string()
            } else {
                theme.red_text("Failed to write file").to_string()
            }
        },
        Err(err) => theme.red_text(&format!("Error as follow: {err}")).to_string(),
    }
}
