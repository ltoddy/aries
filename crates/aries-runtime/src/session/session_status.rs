use crate::session::Session;

#[derive(Debug, Clone)]
pub enum SessionStatus {
    Open,
    Saved,
}

impl SessionStatus {
    pub fn open() -> Self {
        Self::Open
    }

    pub fn saved() -> Self {
        Self::Saved
    }
}

impl Session {
    pub fn session_status(&self) -> SessionStatus {
        SessionStatus::saved()
    }
}
