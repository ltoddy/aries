use agent_client_protocol::Error;
use agent_client_protocol::v1::schema::{
    LoadSessionResponse, NewSessionResponse, PromptCapabilities, SessionConfigId,
    SessionConfigOption, SessionId, SessionInfo, SessionNotification, StopReason,
};

use crate::acp::{DeletionReport, Permission};

#[derive(Debug)]
pub enum Event {
    Connected {
        list_sessions: bool,
        load_session: bool,
        prompt_capabilities: PromptCapabilities,
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
