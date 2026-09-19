use agent_client_protocol::schema::v1::{CloseSessionRequest, CloseSessionResponse};
use agent_client_protocol::{Client, ConnectionTo, Error, Responder};
use aries_session::SharedRegistry;
use tracing::{info, instrument};

#[instrument(name = "acp.close_session", skip_all, fields(session_id = %req.session_id))]
pub async fn close_session(
    req: CloseSessionRequest,
    responder: Responder<CloseSessionResponse>,
    _: ConnectionTo<Client>,
    registry: SharedRegistry,
) -> Result<(), Error> {
    let session_id = req.session_id.to_string();
    info!("Received close session request for {session_id}");

    let mut registry = registry.lock().await;
    registry.close_session(session_id).await;

    let resp = CloseSessionResponse::new();
    responder.respond(resp)
}
