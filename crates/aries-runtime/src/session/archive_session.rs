use crate::session::{Session, SessionError};

impl Session {
    pub async fn archive_session(&mut self) -> Result<(), SessionError> {
        self.session_repo
            .update_archived_by_session_id(&self.state.session_id, true)
            .await
            .map_err(SessionError::db)
    }

    pub async fn unarchive_session(&mut self) -> Result<(), SessionError> {
        self.session_repo
            .update_archived_by_session_id(&self.state.session_id, false)
            .await
            .map_err(SessionError::db)
    }
}
