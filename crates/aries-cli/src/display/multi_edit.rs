use aries_core::tools::multiedit::{MultiEditArgs, MultiEditOutput, NAME};

use crate::display::preview;
use crate::theme::Theme;

pub fn format_tool_call(args: &str, theme: &Theme) -> (String, Option<String>) {
    let args = serde_json::from_str::<MultiEditArgs>(args);

    let (first, rest) = match args {
        Ok(args) => {
            let first = format!("{}", args.file_path.display());

            let mut rest_lines = Vec::new();
            for edit in args.edits {
                let old_lines = edit
                    .old_string
                    .lines()
                    .map(|line| theme.red_text(&format!("- {}", line)).to_string());

                let new_lines = edit
                    .new_string
                    .lines()
                    .map(|line| theme.red_text(&format!("- {}", line)).to_string());

                let diff = old_lines.chain(new_lines).collect::<Vec<_>>().join("\n");
                if !diff.is_empty() {
                    rest_lines.push(preview(diff));
                }
            }

            let rest = if rest_lines.is_empty() { None } else { Some(rest_lines.join("\n")) };
            (first, rest)
        },
        Err(_) => return (String::from("?"), None),
    };

    (format!("{} {}", theme.cyan_text(NAME), theme.yellow_text(&first)), rest)
}

pub fn format_tool_result(result: &str, theme: Theme) -> String {
    let output = serde_json::from_str::<MultiEditOutput>(result);

    match output {
        Ok(output) => theme.dimmed(&preview(output.message)).to_string(),
        Err(err) => theme.red_text(&format!("Error as follow: {err}")).to_string(),
    }
}
