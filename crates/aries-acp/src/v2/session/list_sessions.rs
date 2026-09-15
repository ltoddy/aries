use agent_client_protocol::schema::v2::{ListSessionsRequest, ListSessionsResponse, SessionInfo};
use agent_client_protocol::{Client, Error, Responder, V2ConnectionTo};
use aries_session::SharedRegistry;
use tracing::info;

pub async fn list_sessions(
    req: ListSessionsRequest,
    responder: Responder<ListSessionsResponse>,
    _cx: V2ConnectionTo<Client>,
    registry: SharedRegistry,
) -> Result<(), Error> {
    info!("Received list sessions request (v2): {req:?}");

    let cwd = req.cwd.map(|p| p.into_inner());
    let mut registry = registry.lock().await;
    let sessions = match registry.list_sessions(cwd).await {
        Ok(sessions) => sessions,
        Err(err) => return responder.respond_with_internal_error(err.to_string()),
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
