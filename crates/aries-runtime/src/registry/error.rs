use std::io;
use std::path::PathBuf;

use aries_init::SettingError;

use crate::session::SessionError;

#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    #[error("{0}")]
    Db(#[from] toasty::Error),

    #[error("{0}")]
    Setting(#[from] SettingError),

    #[error("{0}")]
    Session(#[from] SessionError),

    #[error("parent session {0} is not active")]
    SessionNotActive(String),

    #[error("failed to remove session directory {}: {source}", path.display())]
    RemoveSessionDir {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

impl RegistryError {
    pub fn db(source: toasty::Error) -> Self {
        Self::Db(source)
    }

    pub fn setting(source: SettingError) -> Self {
        Self::Setting(source)
    }

    pub fn session(source: SessionError) -> Self {
        Self::Session(source)
    }

    pub fn session_not_active(session_id: impl Into<String>) -> Self {
        Self::SessionNotActive(session_id.into())
    }

    pub fn remove_session_dir(path: impl Into<PathBuf>, source: io::Error) -> Self {
        Self::RemoveSessionDir { path: path.into(), source }
    }
}
