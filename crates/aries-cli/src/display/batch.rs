use aries_core::tools::{BatchArgs, BatchTool};
use aries_theme::Theme;
use rig::providers::openai;
use rig::tool::Tool;

pub fn format_tool_call(args: &str, theme: &Theme) -> (String, Option<String>) {
    const NAME: &str = BatchTool::<openai::CompletionModel, ()>::NAME;

    let args = serde_json::from_str::<BatchArgs>(args);

    let (first, rest) = match args {
        Ok(args) => {
            let first = format!("{} tool calls", args.calls.len());
            let rest = if args.calls.is_empty() {
                None
            } else {
                Some(
                    args.calls
                        .iter()
                        .enumerate()
                        .map(|(i, c)| format!("{}. {}", i + 1, c.tool))
                        .collect::<Vec<_>>()
                        .join("\n"),
                )
            };
            (first, rest)
        },
        Err(_) => return (String::from("?"), None),
    };

    (format!("{} {}", theme.cyan_text(NAME), theme.yellow_text(&first)), rest)
}

pub fn format_tool_result(raw_text: &str, theme: Theme) -> String {
    theme.dimmed(raw_text).to_string()
}
