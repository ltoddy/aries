use agent_client_protocol::schema::v2::{AuthenticateRequest, AuthenticateResponse};
use agent_client_protocol::{Client, ConnectionTo, Error, Responder};
use tracing::info;

pub async fn authenticate(
    req: AuthenticateRequest,
    _responder: Responder<AuthenticateResponse>,
    _cx: ConnectionTo<Client>,
) -> Result<(), Error> {
    info!("Received authenticate request (v2): {req:?}");
    todo!()
}
