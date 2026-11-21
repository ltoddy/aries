use aries_init::SettingError;

use crate::session::SessionError;

#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    #[error("session {0} is already active")]
    SessionAlreadyActive(String),

    #[error("{0}")]
    Db(#[from] toasty::Error),

    #[error("{0}")]
    Setting(#[from] SettingError),

    #[error("{0}")]
    Session(#[from] SessionError),

    #[error("session {0} is not active")]
    SessionNotActive(String),

    #[error("session {0} not found")]
    SessionNotFound(String),
}

impl RegistryError {
    pub fn session_already_active(session_id: impl Into<String>) -> Self {
        Self::SessionAlreadyActive(session_id.into())
    }

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

    pub fn session_not_found(session_id: impl Into<String>) -> Self {
        Self::SessionNotFound(session_id.into())
    }
}
