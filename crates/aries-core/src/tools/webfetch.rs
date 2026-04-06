use anyhow::Result;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
#[allow(dead_code)]
pub struct WebFetchArgs {
    url: String,
    format: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct WebFetchOutput {
    pub content: String,
}

#[derive(thiserror::Error, Debug)]
pub enum WebFetchError {
    #[error("Failed to fetch web content: {0}")]
    FetchError(String),
}

pub struct WebFetchTool;

impl Tool for WebFetchTool {
    const NAME: &'static str = "web_fetch";
    type Error = WebFetchError;
    type Args = WebFetchArgs;
    type Output = WebFetchOutput;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: include_str!("descriptions/webfetch.txt").to_string(),
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
