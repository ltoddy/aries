use aries_core::tools::{GrepArgs, GrepOutput, GrepTool};
use aries_theme::Theme;
use rig::tool::Tool;

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

pub fn format_tool_result(raw_text: &str, theme: Theme) -> String {
    serde_json::from_str::<GrepOutput>(raw_text)
        .map(|output| theme.dimmed(&output.matches.join("\n")).to_string())
        .unwrap_or_else(|_| theme.dimmed(raw_text).to_string())
}
