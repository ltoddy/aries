use aries_core::tools::{WriteFileArgs, WriteFileOutput, WriteFileTool};
use aries_theme::Theme;
use rig::tool::Tool;

use crate::display::preview;

pub fn format_call(args: &str, theme: &Theme) -> String {
    let args = serde_json::from_str::<WriteFileArgs>(args);

    let args = match args {
        Ok(args) => format!("{}\n{}", args.file_path.display(), preview(&args.content)),
        Err(_) => String::from("?"),
    };

    format!("{} {}", theme.cyan_text(WriteFileTool::NAME), theme.yellow_text(&args))
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

#[test]
pub fn test_format_call() {
    use std::path::PathBuf;

    let theme = Theme::default();
    let content = r#"use aries_core::tools::{WriteFileArgs, WriteFileOutput, WriteFileTool};
use aries_theme::Theme;
use rig::tool::Tool;

use super::common::preview;

pub fn format_call(args: &str, theme: &Theme) -> String {
    let args = serde_json::from_str::<WriteFileArgs>(args);

    let args = match args {
        Ok(args) => format!("{}\n{}", args.file_path.display(), preview(&args.content)),
        Err(_) => String::from("?"),
    };

    format!("{} {}", theme.cyan_text(WriteFileTool::NAME), theme.yellow_text(&args))
}"#;

    let args = WriteFileArgs { file_path: PathBuf::from("/home/foo/bar"), content: content.to_owned() };
    let args = serde_json::to_string(&args).unwrap();

    let content = format_call(&args, &theme);
    println!("{content}");
}
