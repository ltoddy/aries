use serde::{Deserialize, Serialize};

use crate::{RenderError, ToolArgsRender};

#[derive(Debug, Deserialize, Serialize)]
pub struct WebSearchArgs {
    pub query: String,
    pub num: Option<i32>,
}

impl WebSearchArgs {
    pub fn title(&self) -> String {
        format!("Search the web for {}", self.query)
    }
}

impl ToolArgsRender for WebSearchArgs {
    fn render_args(raw: &str) -> Result<(String, Option<String>), RenderError> {
        let args: Self = serde_json::from_str(raw)?;
        let first = args.query;
        Ok((first, None))
    }
}
