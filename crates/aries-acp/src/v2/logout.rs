use agent_client_protocol::schema::v2::{LogoutAuthRequest, LogoutAuthResponse};
use agent_client_protocol::{Client, ConnectionTo, Error, Responder};
use tracing::info;

pub async fn logout(
    req: LogoutAuthRequest,
    _responder: Responder<LogoutAuthResponse>,
    _cx: ConnectionTo<Client>,
) -> Result<(), Error> {
    info!("Received logout request (v2): {req:?}");
    todo!()
}
