use agent_client_protocol::v2::ConnectionTo;
use agent_client_protocol::v2::schema::{
    self, CancelRequestNotification, CancelSessionNotification,
};
use agent_client_protocol::{Client, Error};
use aries_runtime::session::SharedRegistry;
use tracing::info;

pub async fn cancel_request(
    notif: CancelRequestNotification,
    _cx: ConnectionTo<Client>,
) -> Result<(), Error> {
    info!("Received cancel request notification (v2): {notif:?}");
    Ok(())
}

pub async fn cancel(
    notif: CancelSessionNotification,
    _cx: ConnectionTo<Client>,
    registry: SharedRegistry,
) -> Result<(), Error> {
    info!("Received cancel notification (v2): {notif:?}");

    let session_id = notif.session_id.to_string();
    let session = {
        let registry = registry.lock().await;
        match registry.session(&session_id) {
            Some(session) => session,
            None => return Err(schema::Error::resource_not_found().into()),
        }
    };
    session.cancel();

    Ok(())
}
