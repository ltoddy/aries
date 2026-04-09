use aries_core::tools::{BatchArgs, BatchTool};
use aries_theme::Theme;
use rig::providers::openai;
use rig::tool::Tool;

pub fn format_call(args: &str, theme: &Theme) -> String {
    const NAME: &str = BatchTool::<openai::CompletionModel, ()>::NAME;

    let args = serde_json::from_str::<BatchArgs>(args);

    let args = match args {
        Ok(args) => format!("{} tool calls", args.calls.len()),
        Err(_) => String::from("?"),
    };

    format!("{} {}", theme.cyan_text(NAME), theme.yellow_text(&args))
}

pub fn format_result(raw_text: &str) -> String {
    raw_text.to_string()
}
