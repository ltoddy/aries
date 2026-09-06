use agent_client_protocol::schema::v1::{SetSessionModeRequest, SetSessionModeResponse};
use agent_client_protocol::schema::v2::{
    CloseSessionRequest, CloseSessionResponse, DeleteSessionRequest, DeleteSessionResponse,
    ForkSessionRequest, ForkSessionResponse, ListSessionsRequest, ListSessionsResponse,
    NewSessionRequest, NewSessionResponse, ResumeSessionRequest, ResumeSessionResponse,
    SetSessionConfigOptionRequest, SetSessionConfigOptionResponse,
};
use agent_client_protocol::{Client, Error, Responder, V2ConnectionTo};
use tracing::info;

pub async fn new_session(
    req: NewSessionRequest,
    _responder: Responder<NewSessionResponse>,
    _cx: V2ConnectionTo<Client>,
) -> Result<(), Error> {
    info!("Received new session request (v2): {req:?}");
    todo!()
}

pub async fn list_sessions(
    req: ListSessionsRequest,
    _responder: Responder<ListSessionsResponse>,
    _cx: V2ConnectionTo<Client>,
) -> Result<(), Error> {
    info!("Received list sessions request (v2): {req:?}");
    todo!()
}

pub async fn delete_session(
    req: DeleteSessionRequest,
    _responder: Responder<DeleteSessionResponse>,
    _cx: V2ConnectionTo<Client>,
) -> Result<(), Error> {
    info!("Received delete session request (v2): {req:?}");
    todo!()
}

pub async fn close_session(
    req: CloseSessionRequest,
    _responder: Responder<CloseSessionResponse>,
    _cx: V2ConnectionTo<Client>,
) -> Result<(), Error> {
    info!("Received close session request (v2): {req:?}");
    todo!()
}

pub async fn resume_session(
    req: ResumeSessionRequest,
    _responder: Responder<ResumeSessionResponse>,
    _cx: V2ConnectionTo<Client>,
) -> Result<(), Error> {
    info!("Received resume session request (v2): {req:?}");
    todo!()
}

pub async fn set_session_config_option(
    req: SetSessionConfigOptionRequest,
    _responder: Responder<SetSessionConfigOptionResponse>,
    _cx: V2ConnectionTo<Client>,
) -> Result<(), Error> {
    info!("Received set session config option request (v2): {req:?}");
    todo!()
}

pub async fn set_session_mode(
    req: SetSessionModeRequest,
    _responder: Responder<SetSessionModeResponse>,
    _cx: V2ConnectionTo<Client>,
) -> Result<(), Error> {
    info!("Received set session mode request (v2): {req:?}");
    todo!()
}

pub async fn fork_session(
    req: ForkSessionRequest,
    _responder: Responder<ForkSessionResponse>,
    _cx: V2ConnectionTo<Client>,
) -> Result<(), Error> {
    info!("Received fork session request (v2): {req:?}");
    todo!()
}
