use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct ReadArgs {
    pub file_path: PathBuf,
    pub offset: Option<usize>,
    pub limit: Option<usize>,
}

impl ReadArgs {
    pub fn location(&self) -> impl Into<PathBuf> {
        &self.file_path
    }

    pub fn title(&self) -> String {
        format!("Read file {}", self.file_path.display())
    }
}

impl ReadArgs {
    pub fn render_args(raw: &str) -> Result<(String, Option<String>), serde_json::Error> {
        let args: Self = serde_json::from_str(raw)?;

        let mut first = format!("{}", args.file_path.display());
        if let Some(offset) = args.offset {
            first.push_str(&format!(", offset = {offset}"));
        }
        if let Some(limit) = args.limit {
            first.push_str(&format!(", limit = {limit}"));
        }

        Ok((first, None))
    }
}
