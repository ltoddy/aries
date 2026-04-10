use aries_core::tools::{GrepArgs, GrepOutput, GrepTool};
use aries_theme::Theme;
use rig::tool::Tool;

use crate::display::preview;

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

    (format!("{} {}", theme.cyan_text(GrepTool::NAME), theme.yellow_text(&first)), None)
}

pub fn format_tool_result(result: &str, theme: Theme) -> String {
    let output = serde_json::from_str::<GrepOutput>(result);

    match output {
        Ok(output) => theme.dimmed(&preview(&output.matches.join("\n"))).to_string(),
        Err(err) => theme.red_text(&format!("Error as follow: {err}")).to_string(),
    }
}
