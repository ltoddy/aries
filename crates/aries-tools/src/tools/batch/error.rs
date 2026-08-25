#[derive(thiserror::Error, Debug)]
pub enum BatchError {
    #[error("nested batch calls are not allowed")]
    NestedBatch,

    #[error("AgentTool is not allowed in batch")]
    AgentNotAllowed,

    #[error("tool '{0}' not found or not supported in batch")]
    UnsupportedTool(String),

    #[error("invalid parameters for tool '{tool}': {source}")]
    InvalidParameters { tool: String, source: serde_json::Error },

    #[error("failed to serialize output for tool '{tool}': {source}")]
    SerializeOutput { tool: String, source: serde_json::Error },

    #[error("tool '{tool}' failed: {message}")]
    ToolExecution { tool: String, message: String },
}

impl BatchError {
    pub fn nested_batch() -> Self {
        Self::NestedBatch
    }

    pub fn agent_not_allowed() -> Self {
        Self::AgentNotAllowed
    }

    pub fn unsupported_tool(tool: impl Into<String>) -> Self {
        Self::UnsupportedTool(tool.into())
    }

    pub fn invalid_parameters(tool: impl Into<String>, source: serde_json::Error) -> Self {
        Self::InvalidParameters { tool: tool.into(), source }
    }

    pub fn serialize_output(tool: impl Into<String>, source: serde_json::Error) -> Self {
        Self::SerializeOutput { tool: tool.into(), source }
    }

    pub fn tool_execution(tool: impl Into<String>, source: impl ToString) -> Self {
        Self::ToolExecution { tool: tool.into(), message: source.to_string() }
    }
}
