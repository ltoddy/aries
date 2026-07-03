use anyhow::Result;
use rig_core::completion::ToolDefinition;
use rig_core::tool::Tool;
use serde::{Deserialize, Serialize};

use crate::tools::{RenderError, ToolArgsRender, ToolOutputRender};

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

#[derive(Debug, Deserialize, Serialize)]
pub struct WebFetchOutput {
    pub content: String,
}

impl ToolOutputRender for WebFetchOutput {
    fn render_output(raw: &str) -> Result<String, RenderError> {
        let output: Self = serde_json::from_str(raw)?;
        Ok(output.content)
    }
}

#[derive(thiserror::Error, Debug)]
pub enum WebFetchError {
    #[error("Failed to fetch web content: {0}")]
    FetchError(String),
}

pub const NAME: &str = "WebFetch";

pub struct WebFetchTool;

impl Tool for WebFetchTool {
    const NAME: &'static str = NAME;
    type Error = WebFetchError;
    type Args = WebFetchArgs;
    type Output = WebFetchOutput;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_owned(),
            description: include_str!("webfetch.md").to_owned(),
            parameters: serde_json::json!({
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
            }),
        }
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

        // For MVP, we just return raw HTML.
        // A full implementation would use a library like `html2md` to convert it.
        let mut truncated = content;
        if truncated.len() > 10000 {
            truncated.truncate(10000);
            truncated.push_str("\n... (content truncated due to length)");
        }

        Ok(WebFetchOutput { content: truncated })
    }
}
