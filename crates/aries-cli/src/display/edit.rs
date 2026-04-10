use aries_core::tools::{EditArgs, EditOutput, EditTool};
use aries_theme::Theme;
use rig::tool::Tool;

pub fn format_call(args: &str, theme: &Theme) -> String {
    let args = serde_json::from_str::<EditArgs>(args);

    let args = match args {
        Ok(args) => {
            let mut content = format!("{}", args.file_path.display());
            if args.replace_all {
                content.push_str(" replace_all = true");
            }

            if !args.old_string.is_empty() || !args.new_string.is_empty() {
                let old_lines = args.old_string.lines().map(|line| format!("- {}", line));
                let new_lines = args.new_string.lines().map(|line| format!("+ {}", line));
                let diff = old_lines.chain(new_lines).collect::<Vec<_>>().join("\n");

                content.push('\n');
                content.push_str(&diff);
            }

            content
        },
        Err(_) => String::from("?"),
    };

    format!("{} {}", theme.cyan_text(EditTool::NAME), theme.yellow_text(&args))
}

pub fn format_result(raw_text: &str) -> String {
    serde_json::from_str::<EditOutput>(raw_text).map(|output| output.message).unwrap_or_else(|_| raw_text.to_string())
}
