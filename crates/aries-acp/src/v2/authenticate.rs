use agent_client_protocol::schema::v2::{LoginAuthRequest, LoginAuthResponse};
use agent_client_protocol::{Client, ConnectionTo, Error, Responder};
use tracing::info;

pub async fn authenticate(
    req: LoginAuthRequest,
    _responder: Responder<LoginAuthResponse>,
    _cx: ConnectionTo<Client>,
) -> Result<(), Error> {
    info!("Received authenticate request (v2): {req:?}");
    todo!()
}
