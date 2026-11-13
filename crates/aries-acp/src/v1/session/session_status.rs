use agent_client_protocol::v1::schema::{
    SessionStatus, SessionStatusRequest, SessionStatusResponse,
};
use agent_client_protocol::v1::{ConnectionTo, Responder};
use agent_client_protocol::{Client, Error};
use tracing::info;

pub async fn session_status(
    req: SessionStatusRequest,
    responder: Responder<SessionStatusResponse>,
    _: ConnectionTo<Client>,
) -> Result<(), Error> {
    info!("Received session status request {req:?}");

    let resp = SessionStatusResponse::new(SessionStatus::Open);
    responder.respond(resp)
}
