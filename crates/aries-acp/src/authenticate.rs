use agent_client_protocol::schema::{AuthenticateRequest, AuthenticateResponse};
use agent_client_protocol::{Client, ConnectionTo, Responder};
use tracing::info;

pub async fn authenticate(
    req: AuthenticateRequest,
    responder: Responder<AuthenticateResponse>,
    _: ConnectionTo<Client>,
) -> Result<(), agent_client_protocol::schema::Error> {
    info!("Received authenticate request {req:?}");
    responder.respond(AuthenticateResponse::new())
}
