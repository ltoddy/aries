use std::path::PathBuf;

use agent_client_protocol::v1::schema::{
    ContentBlock, ForkSessionRequest, LoadSessionRequest, SessionArchiveFilter, SessionId,
    SetSessionConfigOptionRequest, SetSessionTitleRequest,
};

#[derive(Debug)]
pub enum Command {
    NewSession(PathBuf),
    ListSessions(Option<SessionArchiveFilter>),
    SetSessionTitle(Box<SetSessionTitleRequest>),
    ArchiveSession(SessionId),
    UnarchiveSession(SessionId),
    ForkSession(Box<ForkSessionRequest>),
    DeleteSessions(Vec<SessionId>),
    LoadSession(Box<LoadSessionRequest>),
    SetSessionConfigOption(Box<SetSessionConfigOptionRequest>),
    Prompt { session_id: SessionId, content: Vec<ContentBlock> },
    Cancel,
}
