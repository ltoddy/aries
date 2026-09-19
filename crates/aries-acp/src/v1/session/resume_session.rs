use agent_client_protocol::schema::v1::{ResumeSessionRequest, ResumeSessionResponse};
use agent_client_protocol::{Client, ConnectionTo, Error, Responder};
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
