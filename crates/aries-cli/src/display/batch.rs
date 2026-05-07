use aries_core::tools::batch::{BatchArgs, BatchOutput, NAME};

use crate::display::preview;
use crate::theme::Theme;

pub fn format_tool_call(args: &str, theme: &Theme) -> (String, Option<String>) {
    let args = serde_json::from_str::<BatchArgs>(args);

    let (first, rest) = match args {
        Ok(args) => {
            let first = format!("{} tool calls", args.calls.len());

            if args.calls.is_empty() {
                return (first, None);
            }

            let mut rest = Vec::<String>::new();
            for call in args.calls {
                if call.tool == NAME {
                    rest.push(format!(
                        "{} {}",
                        theme.cyan_text(NAME),
                        theme.yellow_text("(nested batch not allowed)")
                    ));
                    continue;
                }

                let (formatted, detail) =
                    super::format_tool_call_args(&call.tool, &call.parameters.to_string(), theme);
                let mut line = formatted;
                if let Some(detail) = detail {
                    line.push_str(&format!("\n{detail}"));
                }
                rest.push(line);
            }

            (first, Some(preview(rest.join("\n"))))
        },
        Err(_) => return (String::from("?"), None),
    };

    (format!("{} {}", theme.cyan_text(NAME), theme.yellow_text(&first)), rest)
}

pub fn format_tool_result(raw_text: &str, theme: Theme) -> String {
    match serde_json::from_str::<BatchOutput>(raw_text) {
        Ok(output) => theme.dimmed(&format!("{} results", output.results.len())).to_string(),
        Err(_) => theme.dimmed(raw_text).to_string(),
    }
}
