use agent_client_protocol::v2::schema::{LoginAuthRequest, LoginAuthResponse};
use agent_client_protocol::v2::{ConnectionTo, Responder};
use agent_client_protocol::{Client, Error};
use tracing::info;

pub async fn authenticate(
    req: LoginAuthRequest,
    responder: Responder<LoginAuthResponse>,
    _cx: ConnectionTo<Client>,
) -> Result<(), Error> {
    info!("Received authenticate request (v2): {req:?}");
    let resp = LoginAuthResponse::new();
    responder.respond(resp)
}
