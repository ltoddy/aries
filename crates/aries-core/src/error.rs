use rig_core::http_client;

#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("Task execution failed: {0}")]
    ExecutionError(String),

    #[error("Failed to create llm client: {0}")]
    Client(#[from] http_client::Error),
}
