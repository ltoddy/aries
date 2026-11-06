use agent_client_protocol::Error;
use agent_client_protocol::v1::Responder;
use agent_client_protocol::v1::schema::{TerminalOutputRequest, TerminalOutputResponse};

use crate::acp::terminal::SharedTerminals;

pub async fn handle(
    request: TerminalOutputRequest,
    responder: Responder<TerminalOutputResponse>,
    terminals: SharedTerminals,
) -> Result<(), Error> {
    responder.respond_with_result(terminals.output(&request.session_id, &request.terminal_id))
}
