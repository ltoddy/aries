use rig_core::http_client;

#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("Task execution failed: {0}")]
    ExecutionError(String),

    #[error("Failed to create llm client: {0}")]
    Client(#[from] http_client::Error),
}

impl AgentError {
    pub fn is_context_exceeded(&self) -> bool {
        let content = self.to_string().to_lowercase();

        const PATTERNS: [&str; 6] = [
            "prompt_too_long",
            "context_length_exceeded",
            "maximum context length",
            "context length exceeded",
            "too many tokens",
            "input is too long",
        ];

        PATTERNS.iter().any(|p| content.contains(p))
    }
}
