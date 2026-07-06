use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::mcp::{McpLoadResult, McpParseError};

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct McpConfig {
    #[serde(rename = "mcpServers", default)]
    pub mcp_servers: HashMap<String, McpServerConfig>,
}

impl McpConfig {
    pub fn new(mcp_servers: HashMap<String, McpServerConfig>) -> Self {
        Self { mcp_servers }
    }

    pub async fn parse(file_path: impl AsRef<Path>) -> McpLoadResult<Self> {
        let config = tokio::fs::read(file_path)
            .await
            .map_err(McpParseError::io)
            .and_then(|v| serde_json::from_slice(&v).map_err(McpParseError::json))?;
        Ok(config)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum McpServerConfig {
    Stdio(StdioConfig),
    Sse(SseConfig),
    Http(HttpConfig),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StdioConfig {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SseConfig {
    pub url: String,
    #[serde(default)]
    pub headers: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HttpConfig {
    pub url: String,
    #[serde(default)]
    pub headers: HashMap<String, String>,
}
