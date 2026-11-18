use std::path::Path;

use gpui_kit::component::input::{EditorState, RopeExt, TabSize};

#[derive(Debug, PartialEq, Eq)]
pub struct EditorStatus {
    pub line: u32,
    pub column: u32,
    pub selection: std::ops::Range<usize>,
    pub lines: usize,
}

impl EditorStatus {
    pub fn new(editor: &EditorState) -> Self {
        let position = editor.cursor_position();
        Self {
            line: position.line + 1,
            column: position.character + 1,
            selection: editor.selected_range(),
            lines: editor.text().lines_len(),
        }
    }
}

pub fn file_language(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "rs" => "rust",
        "sh" | "bash" | "zsh" => "bash",
        "c" | "h" => "c",
        "cc" | "cpp" | "cxx" | "hpp" | "hh" => "cpp",
        "css" => "css",
        "diff" | "patch" => "diff",
        "go" => "go",
        "html" | "htm" => "html",
        "java" => "java",
        "js" | "mjs" | "cjs" | "jsx" => "javascript",
        "json" => "json",
        "md" | "markdown" => "markdown",
        "py" | "pyi" => "python",
        "toml" => "toml",
        "tsx" => "tsx",
        "ts" | "mts" | "cts" => "typescript",
        "yaml" | "yml" => "yaml",
        _ => "plaintext",
    }
}

#[expect(
    clippy::field_reassign_with_default,
    reason = "TabSize only exposes its Default constructor"
)]
pub fn editor_tab_size() -> TabSize {
    let mut tab = TabSize::default();
    tab.tab_size = 4;
    tab
}
