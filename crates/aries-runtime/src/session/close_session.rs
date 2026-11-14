use aries_extension::hook::input::{SessionEndHookInput, SessionEndReason};

use crate::session::Session;

impl Session {
    pub async fn close_session(&self) {
        let input = SessionEndHookInput::new(
            &self.state.session_id,
            &self.state.cwd,
            SessionEndReason::Logout,
        )
        .transcript_path(&self.transcript_path);

        self.hooks_executor.fire_session_end(input).await;
    }
}
