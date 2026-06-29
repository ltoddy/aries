use agent_client_protocol::schema::v1::{LogoutRequest, LogoutResponse};
use agent_client_protocol::{Client, ConnectionTo, Error, Responder};
use tracing::info;

pub async fn logout(
    req: LogoutRequest,
    responder: Responder<LogoutResponse>,
    _: ConnectionTo<Client>,
) -> Result<(), Error> {
    info!("Received logout request: {req:?}");
    responder.respond(LogoutResponse::new())
}
