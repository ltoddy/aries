use aries_core::tools::{ReadFileArgs, ReadFileOutput, ReadFileTool};
use aries_theme::Theme;
use rig::tool::Tool;

pub fn format_call(args: &str, theme: &Theme) -> String {
    const NAME: &str = ReadFileTool::NAME;

    let args = serde_json::from_str::<ReadFileArgs>(args);

    let args = match args {
        Ok(args) => {
            let mut path = format!("{}", args.file_path.display());
            if let Some(offset) = args.offset {
                path.push_str(&format!(", offset = {offset}"));
            }
            path
        },
        Err(_) => String::from("?"),
    };

    format!("{} {}", theme.cyan_text(NAME), theme.yellow_text(&args))
}

pub fn format_result(raw_text: &str) -> String {
    serde_json::from_str::<ReadFileOutput>(raw_text)
        .map(|output| output.content)
        .unwrap_or_else(|_| raw_text.to_string())
}
