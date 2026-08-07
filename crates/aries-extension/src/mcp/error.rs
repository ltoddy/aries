use rig_agent::tool::rmcp::McpClientError;

#[derive(Debug, thiserror::Error)]
pub enum McpParseError {
    #[error("failed to read mcp.json: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to parse mcp.json: {0}")]
    Json(#[from] serde_json::Error),
}

impl McpParseError {
    pub fn io(err: std::io::Error) -> Self {
        McpParseError::Io(err)
    }

    pub fn json(err: serde_json::Error) -> Self {
        McpParseError::Json(err)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum McpConnectError {
    #[error("failed to start MCP stdio server process: {0}")]
    Spawn(std::io::Error),
    #[error("MCP connection timed out after {0:?}")]
    Timeout(std::time::Duration),
    #[error("MCP client error: {0}")]
    Client(#[from] McpClientError),
}

impl McpConnectError {
    pub fn spawn(err: std::io::Error) -> Self {
        Self::Spawn(err)
    }

    pub fn timeout(t: std::time::Duration) -> Self {
        Self::Timeout(t)
    }

    pub fn client(err: McpClientError) -> Self {
        Self::Client(err)
    }
}
