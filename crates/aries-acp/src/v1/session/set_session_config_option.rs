use agent_client_protocol::v1::schema::{
    self, SetSessionConfigOptionRequest, SetSessionConfigOptionResponse,
};
use agent_client_protocol::v1::{ConnectionTo, Responder};
use agent_client_protocol::{Client, Error};
use aries_init::SettingError;
use aries_runtime::registry::SharedRegistry;
use aries_runtime::session::SessionError;
use tracing::{info, instrument};

use crate::v1::session::config::{SessionConfig, config_options};

#[instrument(
    name = "acp.set_session_config_option",
    skip_all,
    fields(session_id = %req.session_id)
)]
pub async fn set_session_config_option(
    req: SetSessionConfigOptionRequest,
    responder: Responder<SetSessionConfigOptionResponse>,
    _: ConnectionTo<Client>,
    registry: &SharedRegistry,
) -> Result<(), Error> {
    info!("Received set session config option request {req:?}");

    let session_id = req.session_id.to_string();
    let config_id = req.config_id.to_string();
    let session_config = match config_id.parse::<SessionConfig>() {
        Ok(config) => config,
        Err(err) => {
            return responder.respond_with_error(Error::invalid_params().data(err.to_string()));
        },
    };
    let Some(value) = req.value.as_value_id() else {
        return responder.respond_with_error(
            Error::invalid_params()
                .data(format!("session config `{config_id}` requires a select option value ID")),
        );
    };
    let value = value.to_string();

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

    let result = match session_config {
        SessionConfig::Mode => session.set_mode(&value).await,
        SessionConfig::Model => session.set_model(value).await,
        SessionConfig::ThoughtLevel => session.set_thought_level(value).await,
    };
    let result = result.map(|()| {
        SetSessionConfigOptionResponse::new(config_options(
            session.setting(),
            session.mode(),
            session.agents(),
            session.thought_level(),
        ))
    });
    registry.lock().await.putback_session(session);

    match result {
        Ok(resp) => responder.respond(resp),
        Err(
            err @ (SessionError::AgentNotFound(_)
            | SessionError::InvalidThoughtLevel(_)
            | SessionError::Setting(SettingError::ActiveModelNotFound(_))),
        ) => responder.respond_with_error(Error::invalid_params().data(err.to_string())),
        Err(err) => responder.respond_with_internal_error(err.to_string()),
    }
}
