use agent_client_protocol::schema::v2::{InitializeRequest, InitializeResponse};
use agent_client_protocol::{Client, Error, Responder, V2ConnectionTo};
use tracing::info;

pub async fn initialize(
    req: InitializeRequest,
    _responder: Responder<InitializeResponse>,
    _cx: V2ConnectionTo<Client>,
) -> Result<(), Error> {
    info!("Received initialize request (v2): {req:?}");
    todo!()
}
