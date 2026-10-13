#[derive(thiserror::Error, Debug)]
pub enum AgentError {
    #[error("subagent stream failed while waiting for model or tool output: {0}")]
    Streaming(#[from] rig::agent::StreamingError),
}

impl AgentError {
    pub fn streaming(err: rig::agent::StreamingError) -> Self {
        Self::Streaming(err)
    }
}
