use agent_client_protocol::schema::v2::{InitializeRequest, InitializeResponse};
use agent_client_protocol::{Client, ConnectionTo, Error, Responder};
use tracing::info;

pub async fn initialize(
    req: InitializeRequest,
    _responder: Responder<InitializeResponse>,
    _cx: ConnectionTo<Client>,
) -> Result<(), Error> {
    info!("Received initialize request (v2): {req:?}");
    todo!()
}
