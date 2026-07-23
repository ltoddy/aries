use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct GrepOutput {
    pub matches: Vec<String>,
    pub truncated: bool,
}

impl GrepOutput {
    pub fn new(matches: Vec<String>, truncated: bool) -> Self {
        Self { matches, truncated }
    }

    pub fn render_output(raw: &str) -> Result<String, serde_json::Error> {
        let output: Self = serde_json::from_str(raw)?;
        if output.matches.is_empty() {
            return Ok("No matches found".to_owned());
        }
        let mut text = output.matches.join("\n");
        if output.truncated {
            text.push_str(
                "\n(Results are truncated. Consider using a more specific path or pattern.)",
            );
        }
        Ok(text)
    }
}
