use aries_core::tools::{GlobArgs, GlobOutput, GlobTool};
use aries_theme::Theme;
use rig::tool::Tool;

pub fn format_call(args: &str, theme: &Theme) -> String {
    let args = serde_json::from_str::<GlobArgs>(args);

    let args = match args {
        Ok(args) => {
            let mut content = args.pattern;
            if let Some(base_dir) = args.base_dir {
                content.push_str(&format!(", base_dir = {}", base_dir.display()));
            }
            content
        },
        Err(_) => String::from("?"),
    };

    format!("{} {}", theme.cyan_text(GlobTool::NAME), theme.yellow_text(&args))
}

pub fn format_result(raw_text: &str) -> String {
    serde_json::from_str::<GlobOutput>(raw_text)
        .map(|output| output.files.join("\n"))
        .unwrap_or_else(|_| raw_text.to_string())
}
