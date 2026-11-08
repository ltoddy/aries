#[derive(thiserror::Error, Debug)]
pub enum AgentError {
    #[error("subagent execution failed: {0}")]
    Execution(#[from] Box<rig::completion::PromptError>),
    #[error("agent not found: {0}")]
    AgentNotFound(String),
}

impl AgentError {
    pub fn execution(err: rig::completion::PromptError) -> Self {
        Self::Execution(Box::new(err))
    }
}
