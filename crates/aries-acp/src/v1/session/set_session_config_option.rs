use agent_client_protocol::v1::schema::{
    self, SetSessionConfigOptionRequest, SetSessionConfigOptionResponse,
};
use agent_client_protocol::v1::{ConnectionTo, Responder};
use agent_client_protocol::{Client, Error};
use aries_runtime::registry::SharedRegistry;
use tracing::{info, instrument};

use crate::v1::session::config::config_options;

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
    let guard = registry.lock().await;
    let Some(mut session) = guard.session(&session_id) else {
        return responder
            .respond_with_error(schema::Error::resource_not_found(Some(session_id)).into());
    };
    drop(guard);

    let config_id = req.config_id.to_string();
    let Some(value) = req.value.as_value_id() else {
        return responder.respond_with_error(
            Error::invalid_params()
                .data(format!("session config `{config_id}` requires a select option value ID")),
        );
    };

    if let Err(err) = session.set_session_config_option(config_id, value.to_string()).await {
        return responder.respond_with_internal_error(err.to_string());
    }

    let resp = SetSessionConfigOptionResponse::new(config_options(
        session.setting(),
        session.mode(),
        session.agents(),
        session.thought_level(),
    ));
    registry.lock().await.putback_session(session);
    responder.respond(resp)
}
