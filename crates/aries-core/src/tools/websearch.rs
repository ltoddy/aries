use anyhow::Result;
use rig_core::completion::ToolDefinition;
use rig_core::tool::Tool;
use serde::{Deserialize, Serialize};

use crate::tools::{RenderError, ToolArgsRender, ToolOutputRender};

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

#[derive(Debug, Deserialize, Serialize)]
pub struct WebSearchOutput {
    pub results: String,
}

impl ToolOutputRender for WebSearchOutput {
    fn render_output(raw: &str) -> Result<String, RenderError> {
        let output: Self = serde_json::from_str(raw)?;
        Ok(output.results)
    }
}

#[derive(thiserror::Error, Debug)]
pub enum WebSearchError {
    #[error("Failed to perform web search: {0}")]
    SearchError(String),
}

pub const NAME: &str = "WebSearch";

pub struct WebSearchTool;

impl Tool for WebSearchTool {
    const NAME: &'static str = NAME;
    type Error = WebSearchError;
    type Args = WebSearchArgs;
    type Output = WebSearchOutput;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: include_str!("descriptions/websearch.txt").to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "The search query to be executed"
                    },
                    "num": {
                        "type": "number",
                        "description": "Maximum number of search results to return (default: 5)"
                    }
                },
                "required": ["query"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        // MVP: WebSearch requires a search API like Tavily, Exa, Google, or DuckDuckGo.
        // For now, we will return a placeholder asking the user to provide an API key
        // for a real implementation.
        Ok(WebSearchOutput {
            results: format!(
                "Web search for '{}' is not fully implemented in this MVP. Please integrate an API like Tavily or Exa to enable real-time web search.",
                args.query
            ),
        })
    }
}
