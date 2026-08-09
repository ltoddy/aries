mod args;
mod error;
mod output;
mod tavily;
#[cfg(test)]
mod tests;

use std::time::Instant;

use itertools::Itertools;
use rig_agent::tool::{Tool, ToolContext};
use serde_json::Value;

pub use self::args::WebSearchArgs;
pub use self::error::WebSearchError;
pub use self::output::{SearchResult, WebSearchOutput};
use self::tavily::{TavilyClient, TavilySearchRequest};

pub const NAME: &str = "WebSearch";

const DEFAULT_MAX_RESULTS: i32 = 15;

pub struct WebSearchTool {
    api_key: String,
    tavily: TavilyClient,
}

impl Default for WebSearchTool {
    fn default() -> Self {
        Self::new()
    }
}

impl WebSearchTool {
    pub fn new() -> Self {
        let api_key = std::env::var("TAVILY_API_KEY").unwrap_or_default();
        let tavily = TavilyClient::new(&api_key);
        Self { api_key, tavily }
    }
}

impl Tool for WebSearchTool {
    const NAME: &'static str = NAME;
    type Args = WebSearchArgs;
    type Output = WebSearchOutput;
    type Error = WebSearchError;

    fn description(&self) -> String {
        if self.api_key.is_empty() {
            return include_str!("description-not-configured.md").to_owned();
        }
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
                    "description": "Maximum number of search results to return (default: 15)"
                },
                "allowed_domains": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Only return results from these domains"
                },
                "blocked_domains": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Exclude results from these domains"
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
        if self.api_key.is_empty() {
            return Err(WebSearchError::not_configured());
        }

        let request = TavilySearchRequest::new(
            &args.query,
            args.num.unwrap_or(DEFAULT_MAX_RESULTS),
            false,
            args.allowed_domains,
            args.blocked_domains,
        );

        let start = Instant::now();
        let response = self.tavily.search(request).await.map_err(|err| {
            println!("tavily error is: {err}");
            WebSearchError::search_error(err)
        })?;
        let elapsed = start.elapsed();

        let results = response
            .results
            .into_iter()
            .map(|r| {
                SearchResult::new(
                    r.title.unwrap_or_default(),
                    r.url.unwrap_or_default(),
                    r.content.unwrap_or_default(),
                )
            })
            .collect_vec();

        let output = WebSearchOutput::new(args.query, results, elapsed.as_secs_f64());
        Ok(output)
    }
}
