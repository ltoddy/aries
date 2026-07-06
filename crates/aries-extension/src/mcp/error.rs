#[derive(Debug, thiserror::Error)]
pub enum McpLoadError {
    #[error("failed to read mcp.json: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to parse mcp.json: {0}")]
    Json(#[from] serde_json::Error),
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
