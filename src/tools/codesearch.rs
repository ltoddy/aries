use anyhow::Result;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct CodeSearchArgs {
    query: String,
    tokens: Option<i32>,
}

#[derive(Serialize)]
pub struct CodeSearchOutput {
    results: String,
}

#[derive(thiserror::Error, Debug)]
pub enum CodeSearchError {
    #[error("Failed to perform code search: {0}")]
    SearchError(String),
}

pub struct CodeSearchTool;

impl Tool for CodeSearchTool {
    const NAME: &'static str = "code_search";
    type Error = CodeSearchError;
    type Args = CodeSearchArgs;
    type Output = CodeSearchOutput;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: include_str!("descriptions/codesearch.txt").to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "The search query to be executed"
                    },
                    "tokens": {
                        "type": "number",
                        "description": "Maximum number of tokens to return (default: 5000)"
                    }
                },
                "required": ["query"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        // MVP: Similar to WebSearch, this requires an external API integration.
        Ok(CodeSearchOutput {
            results: format!(
                "Code search for '{}' is not fully implemented in this MVP. Please integrate an API like Exa Code API to enable real-time code search.",
                args.query
            ),
        })
    }
}
