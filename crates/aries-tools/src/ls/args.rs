use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{RenderError, ToolArgsRender};

#[derive(Debug, Deserialize, Serialize)]
pub struct LsArgs {
    pub path: Option<PathBuf>,
    pub ignore: Option<Vec<String>>,
}

impl LsArgs {
    pub fn title(&self) -> String {
        let path = self.path.as_deref().unwrap_or_else(|| std::path::Path::new("."));
        format!("List the {} directory's contents", path.display())
    }
}

impl ToolArgsRender for LsArgs {
    fn render_args(raw: &str) -> Result<(String, Option<String>), RenderError> {
        let args: Self = serde_json::from_str(raw)?;
        let first = args.path.unwrap_or_else(|| PathBuf::from(".")).display().to_string();
        Ok((first, None))
    }
}
