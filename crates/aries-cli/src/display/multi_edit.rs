use aries_core::tools::{MultiEditArgs, MultiEditOutput, MultiEditTool};
use aries_theme::Theme;
use rig::tool::Tool;

use crate::display::preview;

pub fn format_tool_call(args: &str, theme: &Theme) -> (String, Option<String>) {
    const NAME: &str = MultiEditTool::NAME;
    let args = serde_json::from_str::<MultiEditArgs>(args);

    let (first, rest) = match args {
        Ok(args) => {
            let mut rest_lines = Vec::new();
            for (idx, edit) in args.edits.iter().enumerate() {
                if !rest_lines.is_empty() {
                    rest_lines.push(String::new());
                }
                rest_lines.push(format!("Edit {}", idx + 1));
                if edit.replace_all {
                    rest_lines.push("replace_all = true".to_string());
                }
                if !edit.old_string.is_empty() {
                    for line in edit.old_string.lines() {
                        rest_lines.push(format!("- {}", line));
                    }
                }
                if !edit.new_string.is_empty() {
                    for line in edit.new_string.lines() {
                        rest_lines.push(format!("+ {}", line));
                    }
                }
            }

            let rest = if rest_lines.is_empty() { None } else { Some(rest_lines.join("\n")) };
            (args.file_path.display().to_string(), rest)
        },
        Err(_) => return (String::from("?"), None),
    };

    (format!("{} {}", theme.cyan_text(NAME), theme.yellow_text(&first)), rest)
}

pub fn format_tool_result(result: &str, theme: Theme) -> String {
    let output = serde_json::from_str::<MultiEditOutput>(result);

    match output {
        Ok(output) => theme.dimmed(&preview(&output.message)).to_string(),
        Err(err) => theme.red_text(&format!("Error as follow: {err}")).to_string(),
    }
}
