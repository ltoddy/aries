use agent_client_protocol::v1::schema::{
    SessionStatus, SessionStatusRequest, SessionStatusResponse,
};
use agent_client_protocol::v1::{ConnectionTo, Responder, schema};
use agent_client_protocol::{Client, Error};
use aries_runtime::registry::SharedRegistry;
use aries_runtime::session::{self};
use tracing::info;

pub async fn session_status(
    req: SessionStatusRequest,
    responder: Responder<SessionStatusResponse>,
    _: ConnectionTo<Client>,
    registry: &SharedRegistry,
) -> Result<(), Error> {
    info!("Received session status request {req:?}");

    let session_id = req.session_id.to_string();

    let registry = registry.lock().await;
    let Some(session) = registry.session(&session_id) else {
        return responder
            .respond_with_error(schema::Error::resource_not_found(Some(session_id)).into());
    };

    let status = match session.session_status() {
        session::SessionStatus::Open => SessionStatus::Open,
        session::SessionStatus::Saved => SessionStatus::Saved,
    };

    let resp = SessionStatusResponse::new(status);
    responder.respond(resp)
}
