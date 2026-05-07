use aries_core::tools::lsp::NAME;
use aries_core::tools::{LspArgs, LspOutput};

use crate::display::preview;
use crate::theme::Theme;

pub fn format_tool_call(args: &str, theme: &Theme) -> (String, Option<String>) {
    let args = serde_json::from_str::<LspArgs>(args);

    let first = match args {
        Ok(args) => {
            let mut display = format!("{:?}", args.operation);
            if let Some(path) = args.file_path {
                display.push_str(&format!(" {}", path.display()));
            }
            if let Some(line) = args.line {
                display.push_str(&format!(":{line}"));
            }
            if let Some(character) = args.character {
                display.push_str(&format!(":{character}"));
            }
            if let Some(query) = args.query {
                display.push_str(&format!(" query = {query}"));
            }
            display
        },
        Err(_) => return (String::from("?"), None),
    };

    (format!("{} {}", theme.cyan_text(NAME), theme.yellow_text(&first)), None)
}

pub fn format_tool_result(result: &str, theme: Theme) -> String {
    let output = serde_json::from_str::<LspOutput>(result);

    match output {
        Ok(output) => theme.dimmed(&preview(output.to_string())).to_string(),
        Err(_) => theme.red_text(result).to_string(),
    }
}
