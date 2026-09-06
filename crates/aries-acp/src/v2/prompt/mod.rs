use agent_client_protocol::schema::v2::{PromptRequest, PromptResponse};
use agent_client_protocol::{Client, Error, Responder, V2ConnectionTo};
use tracing::info;

pub async fn prompt(
    req: PromptRequest,
    _responder: Responder<PromptResponse>,
    _cx: V2ConnectionTo<Client>,
) -> Result<(), Error> {
    info!("Received prompt request (v2): {req:?}");
    todo!()
}
