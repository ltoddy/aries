use agent_client_protocol::v1::schema::{AuthenticateRequest, AuthenticateResponse};
use agent_client_protocol::v1::{ConnectionTo, Responder};
use agent_client_protocol::{Client, Error};
use tracing::info;

pub async fn authenticate(
    req: AuthenticateRequest,
    responder: Responder<AuthenticateResponse>,
    _: ConnectionTo<Client>,
) -> Result<(), Error> {
    info!("Received authenticate request {req:?}");
    responder.respond(AuthenticateResponse::new())
}
