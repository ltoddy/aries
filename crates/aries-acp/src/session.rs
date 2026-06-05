use agent_client_protocol::schema::{
    ListSessionsRequest, ListSessionsResponse, LoadSessionRequest, LoadSessionResponse,
    NewSessionRequest, NewSessionResponse, SessionInfo, SessionMode, SessionModeId,
    SessionModeState, SetSessionModeRequest, SetSessionModeResponse,
};
use agent_client_protocol::{Client, ConnectionTo, Error, Responder};
use aries_core::agents::AgentType;
use tracing::info;

use crate::SharedRegistry;

pub async fn new_session(
    req: NewSessionRequest,
    responder: Responder<NewSessionResponse>,
    _: ConnectionTo<Client>,
    registry: SharedRegistry,
) -> Result<(), agent_client_protocol::schema::Error> {
    info!("Received new session request {req:?}");

    let mut reg = registry.lock().await;
    let cwd = req.cwd.display().to_string();
    let session = match reg.new_session(cwd).await {
        Ok(session) => session,
        Err(err) => {
            return responder.respond_with_internal_error(err.to_string());
        },
    };

    let resp = NewSessionResponse::new(session.id()).modes(Some(modes()));
    responder.respond(resp)
}

pub async fn load_session(
    req: LoadSessionRequest,
    responder: Responder<LoadSessionResponse>,
    _: ConnectionTo<Client>,
    registry: SharedRegistry,
) -> Result<(), agent_client_protocol::schema::Error> {
    info!("Received list sessions request {req:?}");

    let session_id = req.session_id.to_string();
    let mut reg = registry.lock().await;
    if let Err(err) = reg.load_session(&session_id).await {
        return responder.respond_with_internal_error(err.to_string());
    }

    let resp = LoadSessionResponse::new().modes(Some(modes()));
    responder.respond(resp)
}

pub async fn list_session(
    req: ListSessionsRequest,
    responder: Responder<ListSessionsResponse>,
    _: ConnectionTo<Client>,
    registry: SharedRegistry,
) -> Result<(), agent_client_protocol::schema::Error> {
    info!("Received list sessions request {req:?}");

    let mut reg = registry.lock().await;
    let sessions = match reg.list_sessions(req.cwd).await {
        Ok(sessions) => sessions,
        Err(err) => {
            return responder.respond_with_internal_error(err.to_string());
        },
    };
    let sessions = sessions
        .into_iter()
        .map(|s| {
            SessionInfo::new(s.session_id, s.cwd)
                .title(s.title)
                .updated_at(s.updated_at.to_string())
        })
        .collect();

    responder.respond(ListSessionsResponse::new(sessions))
}

pub async fn set_session_mode(
    req: SetSessionModeRequest,
    responder: Responder<SetSessionModeResponse>,
    _: ConnectionTo<Client>,
    registry: SharedRegistry,
) -> Result<(), agent_client_protocol::schema::Error> {
    info!("Received set session mode request {req:?}");

    let session_id = req.session_id.to_string();
    let mut session = {
        let reg = registry.lock().await;
        match reg.get_session(&session_id) {
            Some(session) => session,
            None => {
                return responder.respond_with_error(Error::resource_not_found(Some(session_id)));
            },
        }
    };

    let mode_id = req.mode_id.to_string();
    let agent_type = AgentType::from_id(&mode_id);
    if let Err(err) = session.switch_agent(agent_type).await {
        return responder.respond_with_internal_error(err.to_string());
    }

    responder.respond(SetSessionModeResponse::new())
}

fn modes() -> SessionModeState {
    SessionModeState::new(
        SessionModeId::new(AgentType::Build.id()),
        vec![
            SessionMode::new(SessionModeId::new(AgentType::Build.id()), AgentType::Build.name())
                .description(Some(AgentType::Build.description().to_owned())),
            SessionMode::new(SessionModeId::new(AgentType::Plan.id()), AgentType::Plan.name())
                .description(Some(AgentType::Plan.description().to_owned())),
            SessionMode::new(
                SessionModeId::new(AgentType::General.id()),
                AgentType::General.name(),
            )
            .description(Some(AgentType::General.description().to_owned())),
            SessionMode::new(
                SessionModeId::new(AgentType::Explore.id()),
                AgentType::Explore.name(),
            )
            .description(Some(AgentType::Explore.description().to_owned())),
        ],
    )
}
