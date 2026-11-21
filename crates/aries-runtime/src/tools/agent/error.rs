use std::io;
use std::path::PathBuf;

#[derive(thiserror::Error, Debug)]
pub enum AgentError {
    #[error("subagent execution failed: {0}")]
    Execution(#[from] Box<rig::completion::PromptError>),
    #[error("agent not found: {0}")]
    AgentNotFound(String),
    #[error("invalid agent ID {0:?}: expected 21 ASCII letters, digits, underscores or hyphens")]
    InvalidAgentId(String),
    #[error("failed to initialize subagent context persistence in {}: {source}", dir.display())]
    ContextPersistence { dir: PathBuf, source: io::Error },
}

impl AgentError {
    pub fn execution(err: rig::completion::PromptError) -> Self {
        Self::Execution(Box::new(err))
    }

    pub fn agent_not_found(name: impl Into<String>) -> Self {
        Self::AgentNotFound(name.into())
    }

    pub fn invalid_agent_id(agent_id: impl Into<String>) -> Self {
        Self::InvalidAgentId(agent_id.into())
    }

    pub fn context_persistence(dir: impl Into<PathBuf>, source: io::Error) -> Self {
        Self::ContextPersistence { dir: dir.into(), source }
    }
}
