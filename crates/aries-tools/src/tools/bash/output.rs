use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct BashOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

impl BashOutput {
    pub fn render_output(raw: &str) -> Result<String, serde_json::Error> {
        let output: Self = serde_json::from_str(raw)?;
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
