use agent_client_protocol::v1::schema::{AuthStatusRequest, AuthStatusResponse};
use agent_client_protocol::v1::{ConnectionTo, Responder};
use agent_client_protocol::{Client, Error};
use tracing::info;

pub async fn auth_status(
    req: AuthStatusRequest,
    responder: Responder<AuthStatusResponse>,
    _: ConnectionTo<Client>,
) -> Result<(), Error> {
    info!("Received auth status request {req:?}");

    // TODO: Report the current authentication status.
    responder.respond_with_error(Error::internal_error().data("auth/status is not implemented yet"))
}
