use aries_core::tools::{MultiEditArgs, MultiEditOutput, MultiEditTool};
use aries_theme::Theme;
use rig::tool::Tool;

pub fn format_call(args: &str, theme: &Theme) -> String {
    const NAME: &str = MultiEditTool::NAME;
    let args = serde_json::from_str::<MultiEditArgs>(args);

    let args = match args {
        Ok(args) => args.file_path.display().to_string(),
        Err(_) => String::from("?"),
    };

    format!("{} {}", theme.cyan_text(NAME), theme.yellow_text(&args))
}

pub fn format_result(raw_text: &str) -> String {
    serde_json::from_str::<MultiEditOutput>(raw_text)
        .map(|output| output.message)
        .unwrap_or_else(|_| raw_text.to_string())
}
