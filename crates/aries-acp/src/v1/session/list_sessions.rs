use agent_client_protocol::v1::schema::{
    ListSessionsRequest, ListSessionsResponse, SessionArchiveFilter, SessionInfo,
};
use agent_client_protocol::v1::{ConnectionTo, Responder};
use agent_client_protocol::{Client, Error};
use aries_runtime::registry::SharedRegistry;
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
        .filter(|session| match req.archived.as_ref().unwrap_or(&SessionArchiveFilter::Exclude) {
            SessionArchiveFilter::Include => true,
            SessionArchiveFilter::Only => session.archived,
            _ => !session.archived,
        })
        .map(|s| {
            let title = s.title();
            SessionInfo::new(s.session_id, s.cwd)
                .additional_directories(vec![])
                .title(title)
                .updated_at(s.updated_at.to_string())
                .archived(s.archived)
        })
        .collect();

    let resp = ListSessionsResponse::new(sessions);
    responder.respond(resp)
}
