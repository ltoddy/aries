use agent_client_protocol::v1::schema::{
    SessionId, SessionInfo, SessionNotification, SessionUpdate,
};

use crate::chat::model::chat::Chat;

#[derive(Debug)]
pub struct SessionLoad {
    session: SessionInfo,
    chat: Chat,
    tokens: Option<u64>,
}

impl SessionLoad {
    pub fn new(session: SessionInfo) -> Self {
        let mut chat = Chat::default();
        chat.set_title(session.title.clone());
        Self { session, chat, tokens: None }
    }

    pub fn session_id(&self) -> &SessionId {
        &self.session.session_id
    }

    pub fn receive(&mut self, notification: SessionNotification) {
        if notification.session_id != self.session.session_id {
            return;
        }
        if let SessionUpdate::UsageUpdate(usage) = &notification.update {
            self.tokens = Some(usage.used);
        }
        let _ = self.chat.receive(notification);
    }

    pub fn into_parts(self) -> (SessionInfo, Chat, Option<u64>) {
        (self.session, self.chat, self.tokens)
    }
}
