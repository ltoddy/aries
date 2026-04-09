use aries_core::tools::{GrepArgs, GrepOutput, GrepTool};
use aries_theme::Theme;
use rig::tool::Tool;

pub fn format_call(args: &str, theme: &Theme) -> String {
    const NAME: &str = GrepTool::NAME;

    let args = serde_json::from_str::<GrepArgs>(args);

    let args = match args {
        Ok(args) => {
            let mut pattern = args.pattern;
            if let Some(include) = args.include {
                pattern.push_str(&format!(", include = {include}"));
            }
            pattern
        },
        Err(_) => String::from("?"),
    };

    format!("{} {}", theme.cyan_text(NAME), theme.yellow_text(&args))
}

pub fn format_result(raw_text: &str) -> String {
    serde_json::from_str::<GrepOutput>(raw_text)
        .map(|output| output.matches.join("\n"))
        .unwrap_or_else(|_| raw_text.to_string())
}
