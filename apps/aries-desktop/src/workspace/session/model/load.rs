use agent_client_protocol::v1::schema::{
    SessionId, SessionInfo, SessionNotification, SessionUpdate,
};

use crate::workspace::conversation::model::Conversation;

#[derive(Debug)]
pub struct SessionLoad {
    session: SessionInfo,
    conversation: Conversation,
    tokens: Option<u64>,
}

impl SessionLoad {
    pub fn new(session: SessionInfo) -> Self {
        let mut conversation = Conversation::default();
        conversation.set_title(session.title.clone());
        Self { session, conversation, tokens: None }
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
        let _ = self.conversation.receive(notification);
    }

    pub fn into_parts(self) -> (SessionInfo, Conversation, Option<u64>) {
        (self.session, self.conversation, self.tokens)
    }
}
