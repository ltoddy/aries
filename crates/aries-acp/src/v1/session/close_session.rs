use agent_client_protocol::v1::schema::{CloseSessionRequest, CloseSessionResponse};
use agent_client_protocol::v1::{ConnectionTo, Responder, schema};
use agent_client_protocol::{Client, Error};
use aries_runtime::registry::SharedRegistry;
use tracing::{info, instrument};

#[instrument(name = "acp.close_session", skip_all, fields(session_id = %req.session_id))]
pub async fn close_session(
    req: CloseSessionRequest,
    responder: Responder<CloseSessionResponse>,
    _: ConnectionTo<Client>,
    registry: &SharedRegistry,
) -> Result<(), Error> {
    info!("Received close session request {req:?}");

    let session_id = req.session_id.to_string();
    let registry = registry.lock().await;
    let Some(session) = registry.session(&session_id) else {
        return responder
            .respond_with_error(schema::Error::resource_not_found(Some(session_id)).into());
    };

    session.close_session().await;

    let resp = CloseSessionResponse::new();
    responder.respond(resp)
}
