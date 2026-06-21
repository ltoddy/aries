use agent_client_protocol::schema::v1::{AuthenticateRequest, AuthenticateResponse};
use agent_client_protocol::{Client, ConnectionTo, Error, Responder};
use tracing::info;

pub async fn authenticate(
    req: AuthenticateRequest,
    responder: Responder<AuthenticateResponse>,
    _: ConnectionTo<Client>,
) -> Result<(), Error> {
    info!("Received authenticate request {req:?}");
    responder.respond(AuthenticateResponse::new())
}
