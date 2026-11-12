use agent_client_protocol::v1::schema::{DeleteSessionRequest, DeleteSessionResponse};
use agent_client_protocol::v1::{ConnectionTo, Responder};
use agent_client_protocol::{Client, Error};
use aries_runtime::session::SharedRegistry;
use tracing::{info, instrument};

#[instrument(name = "acp.delete_session", skip_all, fields(session_id = %req.session_id))]
pub async fn delete_session(
    req: DeleteSessionRequest,
    responder: Responder<DeleteSessionResponse>,
    _: ConnectionTo<Client>,
    registry: &SharedRegistry,
) -> Result<(), Error> {
    let session_id = req.session_id.to_string();
    info!("Received delete session request for {session_id}");

    let mut registry = registry.lock().await;
    if let Err(err) = registry.delete_session(session_id).await {
        return responder.respond_with_internal_error(err.to_string());
    }

    let resp = DeleteSessionResponse::new();
    responder.respond(resp)
}
