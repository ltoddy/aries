use crate::session::{Session, SessionError};

impl Session {
    pub async fn set_session_title(&mut self, title: impl AsRef<str>) -> Result<(), SessionError> {
        let title = title.as_ref();

        self.session_repo
            .update_title_by_session_id(&self.state.session_id, title)
            .await
            .map_err(SessionError::db)
    }
}
