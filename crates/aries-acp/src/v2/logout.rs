use agent_client_protocol::schema::v2::{LogoutRequest, LogoutResponse};
use agent_client_protocol::{Client, ConnectionTo, Error, Responder};
use tracing::info;

pub async fn logout(
    req: LogoutRequest,
    _responder: Responder<LogoutResponse>,
    _cx: ConnectionTo<Client>,
) -> Result<(), Error> {
    info!("Received logout request (v2): {req:?}");
    todo!()
}
