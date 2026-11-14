use crate::session::{Session, SessionError};

impl Session {
    pub async fn delete_session(&mut self) -> Result<(), SessionError> {
        self.close_session().await;

        tokio::fs::remove_dir_all(&self.state.session_dir)
            .await
            .map_err(|err| SessionError::remove_session_dir(&self.state.session_dir, err))?;
        self.session_repo
            .delete_by_session_id(&self.state.session_id)
            .await
            .map_err(SessionError::db)?;
        Ok(())
    }
}
