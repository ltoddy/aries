use aries_core::tools::{LspArgs, LspOutput, LspTool};
use aries_theme::Theme;
use rig::tool::Tool;

pub fn format_tool_call(args: &str, theme: &Theme) -> (String, Option<String>) {
    const NAME: &str = LspTool::NAME;

    let args = serde_json::from_str::<LspArgs>(args);

    let first = match args {
        Ok(args) => {
            let mut operation = args.operation;
            if let Some(path) = args.file_path {
                operation.push_str(&format!(", filePath = {}", path.display()));
            }
            operation
        },
        Err(_) => return (String::from("?"), None),
    };

    (format!("{} {}", theme.cyan_text(NAME), theme.yellow_text(&first)), None)
}

pub fn format_tool_result(raw_text: &str, theme: Theme) -> String {
    serde_json::from_str::<LspOutput>(raw_text)
        .map(|output| {
            if output.result.is_null() {
                "LSP operation successful".to_string()
            } else if let Some(s) = output.result.as_str() {
                s.to_string()
            } else {
                format!("LSP result: {}", output.result)
            }
        })
        .map(|s| theme.dimmed(&s).to_string())
        .unwrap_or_else(|_| theme.dimmed(raw_text).to_string())
}
