use std::io;

use aries_init::SettingError;

#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("agent not found: {0}")]
    AgentNotFound(String),

    #[error("invalid thought level: {0}")]
    InvalidThoughtLevel(String),

    #[error("{0}")]
    Io(#[source] io::Error),

    #[error("{0}")]
    Setting(#[source] SettingError),
}

impl SessionError {
    pub fn agent_not_found(name: impl Into<String>) -> Self {
        Self::AgentNotFound(name.into())
    }

    pub fn invalid_thought_level(level: impl Into<String>) -> Self {
        Self::InvalidThoughtLevel(level.into())
    }

    pub fn io(error: io::Error) -> Self {
        Self::Io(error)
    }

    pub fn setting(error: SettingError) -> Self {
        Self::Setting(error)
    }
}

impl From<io::Error> for SessionError {
    fn from(error: io::Error) -> Self {
        Self::io(error)
    }
}

impl From<SettingError> for SessionError {
    fn from(error: SettingError) -> Self {
        Self::setting(error)
    }
}
