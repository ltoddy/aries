use aries_core::tools::glob::{GlobArgs, GlobOutput, NAME};

use crate::display::preview;
use crate::theme::Theme;

pub fn format_tool_call(args: &str, theme: &Theme) -> (String, Option<String>) {
    let args = serde_json::from_str::<GlobArgs>(args);

    let first = match args {
        Ok(args) => {
            let mut content = args.pattern;
            if let Some(base_dir) = args.base_dir {
                content.push_str(&format!(", base_dir = {}", base_dir.display()));
            }
            content
        },
        Err(_) => return (String::from("?"), None),
    };

    (format!("{} {}", theme.cyan_text(NAME), theme.yellow_text(&first)), None)
}

pub fn format_tool_result(result: &str, theme: Theme) -> String {
    let output = serde_json::from_str::<GlobOutput>(result);

    match output {
        Ok(output) => theme.dimmed(&preview(output.files.join("\n"))).to_string(),
        Err(err) => theme.red_text(&format!("Error as follow: {err}")).to_string(),
    }
}
