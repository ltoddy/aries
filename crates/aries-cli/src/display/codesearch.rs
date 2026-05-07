use aries_core::tools::codesearch::{CodeSearchArgs, CodeSearchOutput, NAME};

use crate::display::preview;
use crate::theme::Theme;

pub fn format_tool_call(args: &str, theme: &Theme) -> (String, Option<String>) {
    let args = serde_json::from_str::<CodeSearchArgs>(args);

    let first = match args {
        Ok(args) => {
            let mut first = args.query;
            if let Some(token) = args.tokens_num {
                first.push_str(&format!(" token = {token}"));
            }
            first
        },
        Err(err) => format!("? ({err})"),
    };

    (format!("{} {}", theme.cyan_text(NAME), theme.yellow_text(&first)), None)
}

pub fn format_tool_result(raw_text: &str, theme: Theme) -> String {
    let output = serde_json::from_str::<CodeSearchOutput>(raw_text);

    match output {
        Ok(output) => theme.dimmed(&preview(output.to_string())).to_string(),
        Err(_) => theme.red_text(raw_text).to_string(),
    }
}
