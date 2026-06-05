use agent_client_protocol::schema::{LogoutRequest, LogoutResponse};
use agent_client_protocol::{Client, ConnectionTo, Responder};
use tracing::info;

pub async fn logout(
    req: LogoutRequest,
    responder: Responder<LogoutResponse>,
    _: ConnectionTo<Client>,
) -> Result<(), agent_client_protocol::schema::Error> {
    info!("Received logout request: {req:?}");
    responder.respond(LogoutResponse::new())
}
