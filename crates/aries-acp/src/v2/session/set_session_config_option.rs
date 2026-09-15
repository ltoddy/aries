use std::str::FromStr;

use agent_client_protocol::schema::v2::{
    ConfigOptionUpdate, SessionUpdate, SetSessionConfigOptionRequest,
    SetSessionConfigOptionResponse, UpdateSessionNotification,
};
use agent_client_protocol::{Client, Error, Responder, V2ConnectionTo};
use aries_mode::Mode;
use aries_session::SharedRegistry;
use tracing::info;

use super::config::{SessionConfig, config_options};

pub async fn set_session_config_option(
    req: SetSessionConfigOptionRequest,
    responder: Responder<SetSessionConfigOptionResponse>,
    cx: V2ConnectionTo<Client>,
    registry: SharedRegistry,
) -> Result<(), Error> {
    info!("Received set session config option request (v2): {req:?}");

    let session_id = req.session_id.to_string();
    let config_id = req.config_id.to_string();
    let value = req.value.as_id().map(ToString::to_string).unwrap_or_default();

    let mut session = {
        let registry = registry.lock().await;
        match registry.session(&session_id) {
            Some(session) => session,
            None => {
                return responder.respond_with_error(Error::resource_not_found(Some(session_id)));
            },
        }
    };

    if let Ok(session_config) = config_id.parse::<SessionConfig>() {
        match session_config {
            SessionConfig::Mode => {
                let mode = Mode::from_str(&value).unwrap_or_default();
                if let Err(err) = session.set_mode(mode).await {
                    return responder.respond_with_internal_error(err.to_string());
                }
            },
            SessionConfig::Model => {
                if let Err(err) = session.set_model(value).await {
                    return responder.respond_with_internal_error(err.to_string());
                }
            },
        }
    }

    let config_options = config_options(session.setting(), session.mode());
    let _ = cx.send_notification(UpdateSessionNotification::new(
        session_id.clone(),
        SessionUpdate::ConfigOptionUpdate(ConfigOptionUpdate::new(config_options.clone())),
    ));
    registry.lock().await.putback_session(session);

    responder.respond(SetSessionConfigOptionResponse::new(config_options))
}
