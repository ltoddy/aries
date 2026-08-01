use std::path::PathBuf;

use itertools::Itertools;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct GlobOutput {
    pub files: Vec<PathBuf>,
    #[serde(default)]
    pub truncated: bool,
}

impl GlobOutput {
    pub fn new(files: Vec<PathBuf>, truncated: bool) -> Self {
        Self { files, truncated }
    }

    pub fn render_output(raw: serde_json::Value) -> Result<String, serde_json::Error> {
        let output: Self = serde_json::from_value(raw)?;

        if output.files.is_empty() {
            return Ok("No files found".to_owned());
        }

        let mut text = output.files.into_iter().map(|p| p.display().to_string()).join("\n");
        if output.truncated {
            text.push_str(
                "\n(Results are truncated. Consider using a more specific path or pattern.)",
            );
        }
        Ok(text)
    }
}
