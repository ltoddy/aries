use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct GlobOutput {
    pub files: Vec<String>,
    #[serde(default)]
    pub truncated: bool,
}

impl GlobOutput {
    pub fn render_output(raw: &str) -> Result<String, serde_json::Error> {
        let output: Self = serde_json::from_str(raw)?;

        if output.files.is_empty() {
            return Ok("No files found".to_owned());
        }

        let mut text = output.files.join("\n");
        if output.truncated {
            text.push_str(
                "\n(Results are truncated. Consider using a more specific path or pattern.)",
            );
        }
        Ok(text)
    }
}
