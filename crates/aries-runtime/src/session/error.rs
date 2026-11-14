use aries_init::SettingError;
use std::io;
use std::path::PathBuf;

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

    #[error("{0}")]
    Db(#[source] toasty::Error),

    #[error("failed to build model http client: {0}")]
    BuildClient(#[source] reqwest::Error),

    #[error("failed to remove session directory {}: {source}", path.display())]
    RemoveSessionDir {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
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

    pub fn db(error: toasty::Error) -> Self {
        Self::Db(error)
    }

    pub fn build_client(source: reqwest::Error) -> Self {
        Self::BuildClient(source)
    }

    pub fn remove_session_dir(path: impl Into<PathBuf>, source: io::Error) -> Self {
        Self::RemoveSessionDir { path: path.into(), source }
    }
}
