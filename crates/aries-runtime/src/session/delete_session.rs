use crate::session::{Session, SessionError};

impl Session {
    pub async fn delete_session(&mut self) -> Result<(), SessionError> {
        self.close_session().await;

        let session_dir = self.state.session_dir();
        tokio::fs::remove_dir_all(&session_dir)
            .await
            .map_err(|err| SessionError::remove_session_dir(&session_dir, err))?;

        let session_id = self.state.session_id();
        let mut session_repo = self.session_repo.clone();
        let mut subagent_repo = self.subagent_repo.clone();

        tokio::try_join!(
            session_repo.delete_by_session_id(&session_id),
            subagent_repo.delete_by_session_id(&session_id),
        )
        .map_err(SessionError::db)?;

        Ok(())
    }
}
