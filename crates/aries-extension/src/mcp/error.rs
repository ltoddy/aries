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
    #[error("failed to spawn MCP process: {0}")]
    Spawn(#[from] std::io::Error),
    #[error("MCP connection error: {0}")]
    Connection(String),
    #[error("failed to fetch MCP tool list: {0}")]
    Service(#[from] rmcp::ServiceError),
}
