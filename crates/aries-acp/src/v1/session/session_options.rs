use agent_client_protocol::v1::schema::{SessionOptionsRequest, SessionOptionsResponse};
use agent_client_protocol::v1::{ConnectionTo, Responder, schema};
use agent_client_protocol::{Client, Error};
use aries_runtime::registry::SharedRegistry;
use aries_runtime::session::DEFAULT_AGENT_NAME;

use crate::v1::session::config::{config_options, modes};

pub async fn session_options(
    req: SessionOptionsRequest,
    responder: Responder<SessionOptionsResponse>,
    _cx: ConnectionTo<Client>,
    registry: &SharedRegistry,
) -> Result<(), Error> {
    match req.session_id {
        Some(session_id) => {
            let session_id = session_id.to_string();

            let session = {
                let registry = registry.lock().await;
                let Some(session) = registry.session(&session_id) else {
                    return responder.respond_with_error(
                        schema::Error::resource_not_found(Some(session_id)).into(),
                    );
                };
                session
            };

            let snapshot = session.snapshot().await;
            let modes = match modes(DEFAULT_AGENT_NAME, snapshot.agents()) {
                Ok(modes) => modes,
                Err(err) => return responder.respond_with_internal_error(err.to_string()),
            };
            let config_options = config_options(
                snapshot.setting(),
                snapshot.sandbox_mode(),
                snapshot.mode(),
                snapshot.agents(),
                snapshot.thought_level(),
            );

            let resp = SessionOptionsResponse::new().modes(modes).config_options(config_options);
            responder.respond(resp)
        },
        None => {
            let resp = SessionOptionsResponse::new();
            responder.respond(resp)
        },
    }
}
