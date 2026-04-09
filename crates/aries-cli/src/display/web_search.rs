use aries_core::tools::{WebSearchArgs, WebSearchOutput, WebSearchTool};
use aries_theme::Theme;
use rig::tool::Tool;

pub fn format_call(args: &str, theme: &Theme) -> String {
    const NAME: &str = WebSearchTool::NAME;
    let args = serde_json::from_str::<WebSearchArgs>(args);

    let args = match args {
        Ok(args) => args.query,
        Err(_) => String::from("?"),
    };

    format!("{} {}", theme.cyan_text(NAME), theme.yellow_text(&args))
}

pub fn format_result(raw_text: &str) -> String {
    serde_json::from_str::<WebSearchOutput>(raw_text)
        .map(|output| output.results)
        .unwrap_or_else(|_| raw_text.to_string())
}
