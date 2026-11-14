use agent_client_protocol::v1::schema::{UnarchiveSessionRequest, UnarchiveSessionResponse};
use agent_client_protocol::v1::{ConnectionTo, Responder, schema};
use agent_client_protocol::{Client, Error};
use aries_runtime::session::SharedRegistry;
use tracing::info;

pub async fn unarchive_session(
    req: UnarchiveSessionRequest,
    responder: Responder<UnarchiveSessionResponse>,
    _cx: ConnectionTo<Client>,
    registry: &SharedRegistry,
) -> Result<(), Error> {
    info!("Received unarchive session request {req:?}");

    let session_id = req.session_id.to_string();
    let registry = registry.lock().await;
    let Some(mut session) = registry.session(&session_id) else {
        return responder
            .respond_with_error(schema::Error::resource_not_found(Some(session_id)).into());
    };

    if let Err(err) = session.unarchive().await {
        return responder.respond_with_internal_error(err.to_string());
    }

    let resp = UnarchiveSessionResponse::new();
    responder.respond(resp)
}
