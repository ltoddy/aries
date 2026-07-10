use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::mcp::{McpLoadResult, McpParseError};

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct McpDefinition {
    #[serde(rename = "mcpServers", default)]
    pub mcp_servers: HashMap<String, McpServerConfig>,
}

impl McpDefinition {
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
    Stdio(Stdio),
    Sse(Sse),
    Http(Http),
}

impl McpServerConfig {
    pub fn stdio(
        command: impl Into<String>,
        args: Vec<String>,
        env: HashMap<String, String>,
    ) -> Self {
        Self::Stdio(Stdio::new(command, args, env))
    }

    pub fn sse(url: impl Into<String>, headers: HashMap<String, String>) -> Self {
        Self::Sse(Sse::new(url, headers))
    }

    pub fn http(url: impl Into<String>, headers: HashMap<String, String>) -> Self {
        Self::Http(Http::new(url, headers))
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Stdio {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

impl Stdio {
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
pub struct Sse {
    pub url: String,
    #[serde(default)]
    pub headers: HashMap<String, String>,
}

impl Sse {
    pub fn new(url: impl Into<String>, headers: HashMap<String, String>) -> Self {
        let url = url.into();
        Self { url, headers }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Http {
    pub url: String,
    #[serde(default)]
    pub headers: HashMap<String, String>,
}

impl Http {
    pub fn new(url: impl Into<String>, headers: HashMap<String, String>) -> Self {
        let url = url.into();
        Self { url, headers }
    }
}
