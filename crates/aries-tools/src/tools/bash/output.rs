use serde::{Deserialize, Serialize};

const MAX_OUTPUT_CHARS: usize = 30_000;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BashOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

impl BashOutput {
    pub fn new(stdout: impl Into<String>, stderr: impl Into<String>, exit_code: i32) -> Self {
        Self { stdout: truncate(stdout), stderr: truncate(stderr), exit_code }
    }

    pub fn render_output(raw: serde_json::Value) -> Result<String, serde_json::Error> {
        let output: Self = serde_json::from_value(raw)?;
        let mut text = String::new();
        if !output.stdout.is_empty() {
            text.push_str(&output.stdout);
        }
        if !output.stderr.is_empty() {
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str(&format!("stderr: {}", output.stderr));
        }
        if output.exit_code != 0 {
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str(&format!("exit_code: {}", output.exit_code));
        }
        Ok(text)
    }
}

fn truncate(content: impl Into<String>) -> String {
    let content = content.into();

    let mut char_indices = content.char_indices();
    let Some((idx, _)) = char_indices.nth(MAX_OUTPUT_CHARS) else {
        return content;
    };

    let head = &content[..idx];
    let truncated_lines = content[idx..].lines().count();
    format!("{head}\n... [{truncated_lines} lines truncated] ...")
}
