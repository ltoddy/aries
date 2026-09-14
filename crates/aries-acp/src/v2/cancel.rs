use agent_client_protocol::schema::v2::{CancelRequestNotification, CancelSessionNotification};
use agent_client_protocol::{Client, Error, V2ConnectionTo};
use tracing::info;

use super::SharedRegistry;

pub async fn cancel_request(
    notif: CancelRequestNotification,
    _cx: V2ConnectionTo<Client>,
) -> Result<(), Error> {
    info!("Received cancel request notification (v2): {notif:?}");
    Ok(())
}

pub async fn cancel(
    notif: CancelSessionNotification,
    _cx: V2ConnectionTo<Client>,
    registry: SharedRegistry,
) -> Result<(), Error> {
    info!("Received cancel notification (v2): {notif:?}");

    let session_id = notif.session_id.to_string();
    let session = {
        let registry = registry.lock().await;
        match registry.session(&session_id) {
            Some(session) => session,
            None => return Err(Error::resource_not_found(Some(session_id))),
        }
    };
    session.cancel();

    Ok(())
}
