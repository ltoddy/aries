use agent_client_protocol::Error;
use agent_client_protocol::v1::Responder;
use agent_client_protocol::v1::schema::{CreateTerminalRequest, CreateTerminalResponse};

use crate::acp::terminal::SharedTerminals;

pub async fn handle(
    request: CreateTerminalRequest,
    responder: Responder<CreateTerminalResponse>,
    terminals: SharedTerminals,
) -> Result<(), Error> {
    responder.respond_with_result(terminals.create(request).map(CreateTerminalResponse::new))
}
