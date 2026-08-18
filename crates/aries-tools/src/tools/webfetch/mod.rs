mod args;
mod error;
mod output;
#[cfg(test)]
mod tests;

use std::env;

use rig::tool::{Tool, ToolContext};
use serde_json::Value;

pub use self::args::WebFetchArgs;
pub use self::error::WebFetchError;
pub use self::output::WebFetchOutput;

pub const NAME: &str = "WebFetch";

const DEFAULT_FIRECRAWL_API_URL: &str = "https://api.firecrawl.dev";

pub struct WebFetchTool;

impl Default for WebFetchTool {
    fn default() -> Self {
        Self::new()
    }
}

impl WebFetchTool {
    pub fn new() -> Self {
        Self
    }
}

impl Tool for WebFetchTool {
    const NAME: &'static str = NAME;
    type Args = WebFetchArgs;
    type Output = WebFetchOutput;
    type Error = WebFetchError;

    fn description(&self) -> String {
        include_str!("description.md").to_owned()
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "url": {
                    "type": "string",
                    "description": "The URL to fetch content from"
                }
            },
            "required": ["url"]
        })
    }

    async fn call(
        &self,
        _context: &mut ToolContext,
        args: Self::Args,
    ) -> Result<Self::Output, Self::Error> {
        let api_url =
            env::var("FIRECRAWL_API_URL").unwrap_or_else(|_| DEFAULT_FIRECRAWL_API_URL.to_owned());
        let api_key = env::var("FIRECRAWL_API_KEY").ok();

        let client = firecrawl::Client::new_selfhosted(api_url, api_key)
            .map_err(WebFetchError::missing_api_key)?;

        let options = firecrawl::ScrapeOptions {
            origin: Some("aries".to_owned()),
            formats: Some(vec![firecrawl::Format::Markdown]),
            ..Default::default()
        };
        let document = client.scrape(args.url, options).await.map_err(WebFetchError::firecrawl)?;
        let output = WebFetchOutput::new(document.markdown.unwrap_or_default());
        Ok(output)
    }
}
