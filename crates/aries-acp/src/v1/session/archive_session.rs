use agent_client_protocol::v1::schema::{self, ArchiveSessionRequest, ArchiveSessionResponse};
use agent_client_protocol::v1::{ConnectionTo, Responder};
use agent_client_protocol::{Client, Error};
use aries_runtime::session::SharedRegistry;
use tracing::info;

pub async fn archive_session(
    req: ArchiveSessionRequest,
    responder: Responder<ArchiveSessionResponse>,
    _cx: ConnectionTo<Client>,
    registry: &SharedRegistry,
) -> Result<(), Error> {
    info!("Received archive session request {req:?}");

    let session_id = req.session_id.to_string();
    let registry = registry.lock().await;
    let Some(mut session) = registry.session(&session_id) else {
        return responder
            .respond_with_error(schema::Error::resource_not_found(Some(session_id)).into());
    };

    if let Err(err) = session.archive().await {
        return responder.respond_with_internal_error(err.to_string());
    }

    let resp = ArchiveSessionResponse::new();
    responder.respond(resp)
}
