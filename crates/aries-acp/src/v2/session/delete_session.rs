use agent_client_protocol::schema::v2::{DeleteSessionRequest, DeleteSessionResponse};
use agent_client_protocol::{Client, Error, Responder, V2ConnectionTo};
use aries_session::SharedRegistry;
use tracing::info;

pub async fn delete_session(
    req: DeleteSessionRequest,
    responder: Responder<DeleteSessionResponse>,
    _cx: V2ConnectionTo<Client>,
    registry: SharedRegistry,
) -> Result<(), Error> {
    let session_id = req.session_id.to_string();
    info!("Received delete session request (v2) for {session_id}");

    let mut registry = registry.lock().await;
    if let Err(err) = registry.delete_session(session_id).await {
        return responder.respond_with_internal_error(err.to_string());
    }

    responder.respond(DeleteSessionResponse::new())
}
