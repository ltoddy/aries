use aries_core::tools::{GlobArgs, GlobOutput, GlobTool};
use aries_theme::Theme;
use rig::tool::Tool;

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

    (format!("{} {}", theme.cyan_text(GlobTool::NAME), theme.yellow_text(&first)), None)
}

pub fn format_tool_result(raw_text: &str, theme: Theme) -> String {
    serde_json::from_str::<GlobOutput>(raw_text)
        .map(|output| theme.dimmed(&output.files.join("\n")).to_string())
        .unwrap_or_else(|_| theme.dimmed(raw_text).to_string())
}
