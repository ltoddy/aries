#[derive(thiserror::Error, Debug)]
pub enum AgentError {
    #[error("subagent execution failed: {0}")]
    Execution(#[from] rig::completion::PromptError),
}

impl AgentError {
    pub fn execution(err: rig::completion::PromptError) -> Self {
        Self::Execution(err)
    }
}
