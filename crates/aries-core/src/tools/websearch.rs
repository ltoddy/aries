use anyhow::Result;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct WebSearchArgs {
    query: String,
    num: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WebSearchOutput {
    pub results: String,
}

#[derive(thiserror::Error, Debug)]
#[allow(dead_code)]
pub enum WebSearchError {
    #[error("Failed to perform web search: {0}")]
    SearchError(String),
}

pub struct WebSearchTool;

impl Tool for WebSearchTool {
    const NAME: &'static str = "web_search";
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
