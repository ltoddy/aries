mod args;
mod error;
mod output;

use rig_core::tool::Tool;
use serde_json::Value;

pub use self::args::WebFetchArgs;
pub use self::error::WebFetchError;
pub use self::output::WebFetchOutput;

pub const NAME: &str = "WebFetch";

pub struct WebFetchTool;

impl Default for WebFetchTool {
    fn default() -> Self {
        Self::new()
    }
}

impl WebFetchTool {
    pub fn new() -> Self {
        Self {}
    }
}

impl Tool for WebFetchTool {
    const NAME: &'static str = NAME;
    type Error = WebFetchError;
    type Args = WebFetchArgs;
    type Output = WebFetchOutput;

    fn description(&self) -> String {
        include_str!("description.md").to_owned()
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "The URL to fetch content from"
                },
                "format": {
                    "type": "string",
                    "description": "The format to return the content in (markdown, text, or html)",
                    "enum": ["markdown", "text", "html"]
                }
            },
            "required": ["url"]
        })
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let url = if args.url.starts_with("http://") {
            args.url.replace("http://", "https://")
        } else {
            args.url.clone()
        };

        let response =
            reqwest::get(&url).await.map_err(|e| WebFetchError::FetchError(e.to_string()))?;

        let content =
            response.text().await.map_err(|e| WebFetchError::FetchError(e.to_string()))?;

        let mut truncated = content;
        if truncated.len() > 10000 {
            truncated.truncate(10000);
            truncated.push_str("\n... (content truncated due to length)");
        }

        Ok(WebFetchOutput { content: truncated })
    }
}
