use agent_client_protocol::v1::schema::{LogoutRequest, LogoutResponse};
use agent_client_protocol::v1::{ConnectionTo, Responder};
use agent_client_protocol::{Client, Error};
use tracing::info;

pub async fn logout(
    req: LogoutRequest,
    responder: Responder<LogoutResponse>,
    _: ConnectionTo<Client>,
) -> Result<(), Error> {
    info!("Received logout request: {req:?}");
    responder.respond(LogoutResponse::new())
}
