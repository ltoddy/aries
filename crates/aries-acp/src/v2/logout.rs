use agent_client_protocol::v2::schema::{LogoutAuthRequest, LogoutAuthResponse};
use agent_client_protocol::v2::{ConnectionTo, Responder};
use agent_client_protocol::{Client, Error};
use tracing::info;

pub async fn logout(
    req: LogoutAuthRequest,
    responder: Responder<LogoutAuthResponse>,
    _cx: ConnectionTo<Client>,
) -> Result<(), Error> {
    info!("Received logout request (v2): {req:?}");
    let resp = LogoutAuthResponse::new();
    responder.respond(resp)
}
