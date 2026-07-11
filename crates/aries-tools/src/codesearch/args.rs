use serde::{Deserialize, Serialize};

use crate::{RenderError, ToolArgsRender};

#[derive(Debug, Deserialize, Serialize)]
pub struct CodeSearchArgs {
    pub query: String,
    pub tokens_num: Option<i32>,
}

impl CodeSearchArgs {
    pub fn title(&self) -> String {
        format!("Search code context for {}", self.query)
    }
}

impl ToolArgsRender for CodeSearchArgs {
    fn render_args(raw: &str) -> Result<(String, Option<String>), RenderError> {
        let args: Self = serde_json::from_str(raw)?;

        let mut first = args.query;
        if let Some(token) = args.tokens_num {
            first.push_str(&format!(" token = {token}"));
        }

        Ok((first, None))
    }
}
