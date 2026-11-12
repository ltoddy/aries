use agent_client_protocol::v1::schema::{ListSessionsRequest, ListSessionsResponse, SessionInfo};
use agent_client_protocol::v1::{ConnectionTo, Responder};
use agent_client_protocol::{Client, Error};
use aries_runtime::session::SharedRegistry;
use tracing::info;

pub async fn list_session(
    req: ListSessionsRequest,
    responder: Responder<ListSessionsResponse>,
    _: ConnectionTo<Client>,
    registry: &SharedRegistry,
) -> Result<(), Error> {
    info!("Received list sessions request {req:?}");

    let mut registry = registry.lock().await;
    let sessions = match registry.list_sessions(req.cwd).await {
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

    let resp = ListSessionsResponse::new(sessions);
    responder.respond(resp)
}
