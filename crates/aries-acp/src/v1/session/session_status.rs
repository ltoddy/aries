use agent_client_protocol::v1::schema::{SessionStatusRequest, SessionStatusResponse};
use agent_client_protocol::v1::{ConnectionTo, Responder};
use agent_client_protocol::{Client, Error};
use tracing::info;

pub async fn session_status(
    req: SessionStatusRequest,
    responder: Responder<SessionStatusResponse>,
    _: ConnectionTo<Client>,
) -> Result<(), Error> {
    info!("Received session status request {req:?}");

    // TODO: Report the requested session's status.
    responder
        .respond_with_error(Error::internal_error().data("session/status is not implemented yet"))
}
