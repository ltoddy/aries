use agent_client_protocol::v2::schema::{CloseSessionRequest, CloseSessionResponse};
use agent_client_protocol::v2::{ConnectionTo, Responder};
use agent_client_protocol::{Client, Error};
use aries_runtime::session::SharedRegistry;
use tracing::info;

pub async fn close_session(
    req: CloseSessionRequest,
    responder: Responder<CloseSessionResponse>,
    _cx: ConnectionTo<Client>,
    registry: SharedRegistry,
) -> Result<(), Error> {
    let session_id = req.session_id.to_string();
    info!("Received close session request (v2) for {session_id}");

    let mut registry = registry.lock().await;
    registry.close_session(session_id).await;

    responder.respond(CloseSessionResponse::new())
}
