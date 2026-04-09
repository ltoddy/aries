use aries_core::tools::{WebFetchArgs, WebFetchOutput, WebFetchTool};
use aries_theme::Theme;
use rig::tool::Tool;

pub fn format_call(args: &str, theme: &Theme) -> String {
    const NAME: &str = WebFetchTool::NAME;
    let args = serde_json::from_str::<WebFetchArgs>(args);

    let args = match args {
        Ok(args) => args.url,
        Err(_) => String::from("?"),
    };

    format!("{} {}", theme.cyan_text(NAME), theme.yellow_text(&args))
}

pub fn format_result(raw_text: &str) -> String {
    serde_json::from_str::<WebFetchOutput>(raw_text)
        .map(|output| output.content)
        .unwrap_or_else(|_| raw_text.to_string())
}
