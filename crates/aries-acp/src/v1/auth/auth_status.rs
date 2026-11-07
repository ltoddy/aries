use agent_client_protocol::v1::schema::{AuthState, AuthStatusRequest, AuthStatusResponse};
use agent_client_protocol::v1::{ConnectionTo, Responder};
use agent_client_protocol::{Client, Error};
use tracing::info;

pub async fn auth_status(
    req: AuthStatusRequest,
    responder: Responder<AuthStatusResponse>,
    _: ConnectionTo<Client>,
) -> Result<(), Error> {
    info!("Received auth status request {req:?}");

    let resp = AuthStatusResponse::new(AuthState::NotRequired);
    responder.respond(resp)
}
