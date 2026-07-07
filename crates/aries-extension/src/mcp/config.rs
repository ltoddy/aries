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

    pub fn empty() -> Self {
        Self { mcp_servers: HashMap::new() }
    }

    pub async fn parse(file_path: impl AsRef<Path>) -> McpLoadResult<Self> {
        let config = tokio::fs::read(file_path)
            .await
            .map_err(McpParseError::io)
            .and_then(|v| serde_json::from_slice(&v).map_err(McpParseError::json))?;
        Ok(config)
    }

    pub fn update(&mut self, other: Self) {
        self.mcp_servers.extend(other.mcp_servers);
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum McpServerConfig {
    Stdio(StdioConfig),
    Sse(SseConfig),
    Http(HttpConfig),
}

impl McpServerConfig {
    pub fn stdio(
        command: impl Into<String>,
        args: Vec<String>,
        env: HashMap<String, String>,
    ) -> Self {
        Self::Stdio(StdioConfig::new(command, args, env))
    }

    pub fn sse(url: impl Into<String>, headers: HashMap<String, String>) -> Self {
        Self::Sse(SseConfig::new(url, headers))
    }

    pub fn http(url: impl Into<String>, headers: HashMap<String, String>) -> Self {
        Self::Http(HttpConfig::new(url, headers))
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StdioConfig {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

impl StdioConfig {
    pub fn new(
        command: impl Into<String>,
        args: Vec<String>,
        env: HashMap<String, String>,
    ) -> Self {
        let command = command.into();
        Self { command, args, env }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SseConfig {
    pub url: String,
    #[serde(default)]
    pub headers: HashMap<String, String>,
}

impl SseConfig {
    pub fn new(url: impl Into<String>, headers: HashMap<String, String>) -> Self {
        let url = url.into();
        Self { url, headers }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HttpConfig {
    pub url: String,
    #[serde(default)]
    pub headers: HashMap<String, String>,
}

impl HttpConfig {
    pub fn new(url: impl Into<String>, headers: HashMap<String, String>) -> Self {
        let url = url.into();
        Self { url, headers }
    }
}
