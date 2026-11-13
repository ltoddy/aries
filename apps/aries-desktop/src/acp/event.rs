use agent_client_protocol::Error;
use agent_client_protocol::v1::schema::{
    ForkSessionResponse, LoadSessionResponse, NewSessionResponse, PromptCapabilities,
    SessionCapabilities, SessionConfigId, SessionConfigOption, SessionId, SessionInfo,
    SessionNotification, StopReason,
};

use crate::acp::{DeletionReport, Permission};

#[derive(Debug)]
pub enum Event {
    Connected {
        list_sessions: bool,
        load_session: bool,
        prompt_capabilities: PromptCapabilities,
        session_capabilities: Box<SessionCapabilities>,
    },
    SessionTitleSet {
        session_id: SessionId,
        result: Result<String, Error>,
    },
    SessionArchived {
        session_id: SessionId,
        archived: bool,
        result: Result<(), Error>,
    },
    SessionForked {
        session_id: SessionId,
        result: Result<ForkSessionResponse, Error>,
    },
    Sessions(Result<Vec<SessionInfo>, Error>),
    SessionsDeleted(DeletionReport),
    SessionLoaded {
        session_id: SessionId,
        result: Result<LoadSessionResponse, Error>,
    },
    SessionConfigSet {
        session_id: SessionId,
        config_id: SessionConfigId,
        result: Result<Vec<SessionConfigOption>, Error>,
    },
    Ready(Box<NewSessionResponse>),
    Update(Box<SessionNotification>),
    Permission(Permission),
    Finished(Result<StopReason, Error>),
    Disconnected(String),
}

impl Event {
    pub fn connected(
        list_sessions: bool,
        load_session: bool,
        prompt_capabilities: PromptCapabilities,
        session_capabilities: SessionCapabilities,
    ) -> Self {
        Self::Connected {
            list_sessions,
            load_session,
            prompt_capabilities,
            session_capabilities: Box::new(session_capabilities),
        }
    }

    pub fn session_title_set(session_id: SessionId, result: Result<String, Error>) -> Self {
        Self::SessionTitleSet { session_id, result }
    }

    pub fn session_archived(
        session_id: SessionId,
        archived: bool,
        result: Result<(), Error>,
    ) -> Self {
        Self::SessionArchived { session_id, archived, result }
    }

    pub fn session_forked(
        session_id: SessionId,
        result: Result<ForkSessionResponse, Error>,
    ) -> Self {
        Self::SessionForked { session_id, result }
    }
}
