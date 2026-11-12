mod args;
mod error;
mod output;

use rig::tool::{Tool, ToolContext};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use self::args::CodeSearchArgs;
pub use self::error::CodeSearchError;
pub use self::output::CodeSearchOutput;

pub const NAME: &str = "CodeSearch";

pub struct CodeSearchTool;

impl Default for CodeSearchTool {
    fn default() -> Self {
        Self::new()
    }
}

impl CodeSearchTool {
    pub fn new() -> Self {
        Self {}
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct McpCodeRequestArgs {
    query: String,
    #[serde(rename = "tokensNum")]
    tokens_num: i32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct McpCodeRequestParams {
    name: String,
    arguments: McpCodeRequestArgs,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct McpCodeRequest {
    jsonrpc: String,
    id: i32,
    method: String,
    params: McpCodeRequestParams,
}

impl McpCodeRequestArgs {
    fn new(query: impl Into<String>, tokens_num: i32) -> Self {
        let query = query.into();
        Self { query, tokens_num }
    }
}

impl McpCodeRequestParams {
    fn new(arguments: McpCodeRequestArgs) -> Self {
        Self { name: "get_code_context_exa".to_owned(), arguments }
    }
}

impl McpCodeRequest {
    fn new(query: impl Into<String>, tokens_num: i32) -> Self {
        let arguments = McpCodeRequestArgs::new(query, tokens_num);
        Self {
            jsonrpc: "2.0".to_owned(),
            id: 1,
            method: "tools/call".to_owned(),
            params: McpCodeRequestParams::new(arguments),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct McpCodeResponseContent {
    #[serde(rename = "type")]
    content_type: String,
    text: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct McpCodeResponseResult {
    content: Vec<McpCodeResponseContent>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct McpCodeResponse {
    jsonrpc: String,
    result: Option<McpCodeResponseResult>,
}

impl Tool for CodeSearchTool {
    const NAME: &'static str = NAME;
    type Args = CodeSearchArgs;
    type Output = CodeSearchOutput;
    type Error = CodeSearchError;

    fn description(&self) -> String {
        include_str!("description.md").to_owned()
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Question about a library, SDK or API. Include relevant names, task and version when needed, e.g. React useState functional update examples."
                },
                "tokens_num": {
                    "type": "number",
                    "description": "Requested content token budget, as an integer. Defaults to 5000; actual returned length and accepted limits depend on the remote service."
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
        let req = McpCodeRequest::new(&args.query, args.tokens_num.unwrap_or(5000));

        let client = reqwest::Client::new();
        let response = client
            .post("https://mcp.exa.ai/mcp")
            .header("accept", "application/json, text/event-stream")
            .header("content-type", "application/json")
            .json(&req)
            .send()
            .await
            .map_err(|e| CodeSearchError::request(&args.query, e))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(CodeSearchError::status(&args.query, status, text));
        }

        let response_text =
            response.text().await.map_err(|e| CodeSearchError::read_body(&args.query, e))?;

        for line in response_text.lines() {
            if let Some(stripped) = line.strip_prefix("data: ")
                && let Ok(data) = serde_json::from_str::<McpCodeResponse>(stripped)
                && let Some(result) = data.result
                && let Some(content) = result.content.into_iter().next()
            {
                return Ok(CodeSearchOutput::new(content.text));
            }
        }

        Ok(CodeSearchOutput::new(
            "No code snippets or documentation found. Please try a different query, be more \
             specific about the library or programming concept, or check the spelling of framework \
             names.",
        ))
    }
}
