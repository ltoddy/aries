use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WriteArgs {
    pub file_path: PathBuf,
    pub content: String,
}

impl WriteArgs {
    pub fn location(&self) -> &Path {
        &self.file_path
    }

    pub fn title(&self) -> String {
        format!("Write file {}", self.file_path.display())
    }
}

impl WriteArgs {
    pub fn render_args(
        raw: impl AsRef<str>,
    ) -> Result<(String, Option<String>), serde_json::Error> {
        let args: Self = serde_json::from_str(raw.as_ref())?;
        let first = args.file_path.display().to_string();
        let rest = Some(args.content);
        Ok((first, rest))
    }
}
