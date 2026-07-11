use serde::{Deserialize, Serialize};

use crate::{RenderError, ToolArgsRender};

#[derive(Debug, Deserialize, Serialize)]
pub struct WebFetchArgs {
    pub url: String,
    pub format: Option<String>,
}

impl WebFetchArgs {
    pub fn title(&self) -> String {
        format!("Fetch URL {}", self.url)
    }
}

impl ToolArgsRender for WebFetchArgs {
    fn render_args(raw: &str) -> Result<(String, Option<String>), RenderError> {
        let args: Self = serde_json::from_str(raw)?;
        let first = args.url;
        Ok((first, None))
    }
}
