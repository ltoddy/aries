use aries_core::tools::{WriteFileArgs, WriteFileOutput, WriteFileTool};
use aries_theme::Theme;
use rig::tool::Tool;

pub fn format_call(args: &str, theme: &Theme) -> String {
    const NAME: &str = WriteFileTool::NAME;
    let args = serde_json::from_str::<WriteFileArgs>(args);

    let args = match args {
        Ok(args) => args.file_path.display().to_string(),
        Err(_) => String::from("?"),
    };

    format!("{} {}", theme.cyan_text(NAME), theme.yellow_text(&args))
}

pub fn format_result(raw_text: &str) -> String {
    serde_json::from_str::<WriteFileOutput>(raw_text)
        .map(
            |output| {
                if output.success {
                    "File written successfully".to_string()
                } else {
                    "Failed to write file".to_string()
                }
            },
        )
        .unwrap_or_else(|_| raw_text.to_string())
}
