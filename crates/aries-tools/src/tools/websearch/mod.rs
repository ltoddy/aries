mod args;
mod error;
mod output;

use rig_agent::tool::{Tool, ToolContext};
use serde_json::Value;

pub use self::args::WebSearchArgs;
pub use self::error::WebSearchError;
pub use self::output::WebSearchOutput;

pub const NAME: &str = "WebSearch";

pub struct WebSearchTool;

impl Default for WebSearchTool {
    fn default() -> Self {
        Self::new()
    }
}

impl WebSearchTool {
    pub fn new() -> Self {
        Self {}
    }
}

impl Tool for WebSearchTool {
    const NAME: &'static str = NAME;
    type Args = WebSearchArgs;
    type Output = WebSearchOutput;
    type Error = WebSearchError;

    fn description(&self) -> String {
        include_str!("description.md").to_owned()
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
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
        })
    }

    async fn call(
        &self,
        _context: &mut ToolContext,
        args: Self::Args,
    ) -> Result<Self::Output, Self::Error> {
        Ok(WebSearchOutput {
            results: format!(
                "Web search for '{}' is not fully implemented in this MVP. Please integrate an API like Tavily or Exa to enable real-time web search.",
                args.query
            ),
        })
    }
}
