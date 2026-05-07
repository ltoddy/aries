use std::fmt::{self, Display};

use anyhow::Result;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct CodeSearchArgs {
    pub query: String,
    pub tokens_num: Option<i32>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CodeSearchOutput {
    pub results: String,
}

impl Display for CodeSearchOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.results)
    }
}

#[derive(thiserror::Error, Debug)]
pub enum CodeSearchError {
    #[error("Failed to perform code search: {0}")]
    SearchError(String),
}

pub const NAME: &str = "CodeSearch";

pub struct CodeSearchTool;

#[derive(Debug, Deserialize, Serialize)]
struct McpCodeRequestArgs {
    query: String,
    #[serde(rename = "tokensNum")]
    tokens_num: i32,
}

#[derive(Debug, Deserialize, Serialize)]
struct McpCodeRequestParams {
    name: String,
    arguments: McpCodeRequestArgs,
}

#[derive(Debug, Deserialize, Serialize)]
struct McpCodeRequest {
    jsonrpc: String,
    id: i32,
    method: String,
    params: McpCodeRequestParams,
}

#[derive(Debug, Deserialize, Serialize)]
struct McpCodeResponseContent {
    #[serde(rename = "type")]
    content_type: String,
    text: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct McpCodeResponseResult {
    content: Vec<McpCodeResponseContent>,
}

#[derive(Debug, Deserialize, Serialize)]
struct McpCodeResponse {
    jsonrpc: String,
    result: Option<McpCodeResponseResult>,
}

impl Tool for CodeSearchTool {
    const NAME: &'static str = NAME;
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
                        "description": "Search query to find relevant context for APIs, Libraries, and SDKs. For example, 'React useState hook examples', 'Python pandas dataframe filtering'."
                    },
                    "tokens_num": {
                        "type": "number",
                        "description": "Number of tokens to return (1000-50000). Default is 5000 tokens."
                    }
                },
                "required": ["query"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let req = McpCodeRequest {
            jsonrpc: "2.0".to_string(),
            id: 1,
            method: "tools/call".to_string(),
            params: McpCodeRequestParams {
                name: "get_code_context_exa".to_string(),
                arguments: McpCodeRequestArgs {
                    query: args.query.clone(),
                    tokens_num: args.tokens_num.unwrap_or(5000),
                },
            },
        };

        let client = reqwest::Client::new();
        let response = client
            .post("https://mcp.exa.ai/mcp")
            .header("accept", "application/json, text/event-stream")
            .header("content-type", "application/json")
            .json(&req)
            .send()
            .await
            .map_err(|e| CodeSearchError::SearchError(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(CodeSearchError::SearchError(format!(
                "Code search error ({}): {}",
                status, text
            )));
        }

        let response_text =
            response.text().await.map_err(|e| CodeSearchError::SearchError(e.to_string()))?;

        // Parse SSE response
        for line in response_text.lines() {
            if let Some(stripped) = line.strip_prefix("data: ")
                && let Ok(data) = serde_json::from_str::<McpCodeResponse>(stripped)
                && let Some(result) = data.result
                && !result.content.is_empty()
            {
                return Ok(CodeSearchOutput { results: result.content[0].text.clone() });
            }
        }

        Ok(CodeSearchOutput {
            results: "No code snippets or documentation found. Please try a different query, be more specific about the library or programming concept, or check the spelling of framework names.".to_string(),
        })
    }
}
