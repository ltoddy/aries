use agent_client_protocol::v1::schema::{CloseSessionRequest, CloseSessionResponse};
use agent_client_protocol::v1::{ConnectionTo, Responder};
use agent_client_protocol::{Client, Error};
use aries_runtime::session::SharedRegistry;
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
