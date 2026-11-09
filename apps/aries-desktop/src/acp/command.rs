use std::path::PathBuf;

use agent_client_protocol::v1::schema::{
    LoadSessionRequest, SessionId, SetSessionConfigOptionRequest,
};

#[derive(Debug)]
pub enum Command {
    NewSession(PathBuf),
    ListSessions,
    DeleteSessions(Vec<SessionId>),
    LoadSession(Box<LoadSessionRequest>),
    SetSessionConfigOption(Box<SetSessionConfigOptionRequest>),
    Prompt { session_id: SessionId, text: String },
    Cancel,
}
