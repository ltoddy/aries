use aries_core::tools::{ApplyPatchArgs, ApplyPatchOutput, ApplyPatchTool};
use aries_theme::Theme;
use rig::tool::Tool;

pub fn format_call(args: &str, theme: &Theme) -> String {
    const NAME: &str = ApplyPatchTool::NAME;
    let args = serde_json::from_str::<ApplyPatchArgs>(args);

    let args = match args {
        Ok(args) => args
            .patch
            .lines()
            .find_map(|line| line.strip_prefix("+++ b/").or_else(|| line.strip_prefix("--- a/")))
            .map(ToString::to_string)
            .unwrap_or(args.patch),
        Err(_) => String::from("?"),
    };

    format!("{} {}", theme.cyan_text(NAME), theme.yellow_text(&args))
}

pub fn format_result(raw_text: &str) -> String {
    serde_json::from_str::<ApplyPatchOutput>(raw_text)
        .map(|output| output.message)
        .unwrap_or_else(|_| raw_text.to_string())
}
