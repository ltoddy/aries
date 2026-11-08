use agent_client_protocol::v1::schema::{
    self, SetSessionConfigOptionRequest, SetSessionConfigOptionResponse,
};
use agent_client_protocol::v1::{ConnectionTo, Responder};
use agent_client_protocol::{Client, Error};
use aries_runtime::session::SharedRegistry;
use tracing::{info, instrument};

use super::config::{SessionConfig, config_options};

#[instrument(
    name = "acp.set_session_config_option",
    skip_all,
    fields(session_id = %req.session_id)
)]
pub async fn set_session_config_option(
    req: SetSessionConfigOptionRequest,
    responder: Responder<SetSessionConfigOptionResponse>,
    _: ConnectionTo<Client>,
    registry: SharedRegistry,
) -> Result<(), Error> {
    info!("Received set session config option request {req:?}");

    let session_id = req.session_id.to_string();
    let config_id = req.config_id.to_string();
    let value = req.value.as_value_id().map(|v| v.to_string()).unwrap_or_default();

    let mut session = {
        let registry = registry.lock().await;
        match registry.session(&session_id) {
            Some(session) => session,
            None => {
                return responder.respond_with_error(
                    schema::Error::resource_not_found(Some(session_id)).into(),
                );
            },
        }
    };

    if let Ok(session_config) = config_id.parse::<SessionConfig>() {
        return match session_config {
            SessionConfig::Mode => {
                let mode = value;
                if let Err(err) = session.set_mode(&mode).await {
                    return responder.respond_with_internal_error(err.to_string());
                }
                let resp = SetSessionConfigOptionResponse::new(config_options(
                    session.setting(),
                    session.mode().to_string(),
                    session.agents(),
                ));
                registry.lock().await.putback_session(session);
                responder.respond(resp)
            },
            SessionConfig::Model => {
                if let Err(err) = session.set_model(value).await {
                    return responder.respond_with_internal_error(err.to_string());
                };
                let resp = SetSessionConfigOptionResponse::new(config_options(
                    session.setting(),
                    session.mode().to_string(),
                    session.agents(),
                ));
                registry.lock().await.putback_session(session);
                responder.respond(resp)
            },
        };
    }

    let resp = SetSessionConfigOptionResponse::new(config_options(
        session.setting(),
        session.mode().to_string(),
        session.agents(),
    ));
    responder.respond(resp)
}
