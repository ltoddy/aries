use agent_client_protocol::v1::schema::{ResumeSessionRequest, ResumeSessionResponse};
use agent_client_protocol::v1::{ConnectionTo, Responder};
use agent_client_protocol::{Client, Error};
use tracing::info;

pub async fn resume_session(
    req: ResumeSessionRequest,
    responder: Responder<ResumeSessionResponse>,
    _: ConnectionTo<Client>,
) -> Result<(), Error> {
    info!("Received resume session request {req:?}");

    let resp = ResumeSessionResponse::new();
    responder.respond(resp)
}
