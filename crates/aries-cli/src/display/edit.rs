use aries_core::tools::edit::{EditArgs, EditOutput, NAME};
use aries_theme::Theme;

use crate::display::preview;

pub fn format_tool_call(args: &str, theme: &Theme) -> (String, Option<String>) {
    let args = serde_json::from_str::<EditArgs>(args);

    let (first, rest) = match args {
        Ok(args) => {
            let mut first = format!("{}", args.file_path.display());
            if args.replace_all {
                first.push_str(" replace_all = true");
            }

            let mut rest = None;
            if !args.old_string.is_empty() || !args.new_string.is_empty() {
                let old_lines = args.old_string.lines().map(|line| {
                    let s = format!("- {}", line);
                    theme.red_text(&s).to_string()
                });
                let new_lines = args.new_string.lines().map(|line| {
                    let s = format!("+ {}", line);
                    theme.green_text(&s).to_string()
                });
                let diff = old_lines.chain(new_lines).collect::<Vec<_>>().join("\n");
                rest = Some(diff);
            }

            (first, rest)
        },
        Err(_) => return (String::from("?"), None),
    };

    (format!("{} {}", theme.cyan_text(NAME), theme.yellow_text(&first)), rest)
}

pub fn format_tool_result(result: &str, theme: Theme) -> String {
    let output = serde_json::from_str::<EditOutput>(result);

    match output {
        Ok(output) => theme.dimmed(&preview(&output.message)).to_string(),
        Err(err) => theme.red_text(&format!("Error as follow: {err}")).to_string(),
    }
}
