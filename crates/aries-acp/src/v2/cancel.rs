use agent_client_protocol::schema::v2::{CancelRequestNotification, CancelSessionNotification};
use agent_client_protocol::{Client, Error, V2ConnectionTo};
use tracing::info;

pub async fn cancel_request(
    notif: CancelRequestNotification,
    _cx: V2ConnectionTo<Client>,
) -> Result<(), Error> {
    info!("Received cancel request notification (v2): {notif:?}");
    todo!()
}

pub async fn cancel(
    notif: CancelSessionNotification,
    _cx: V2ConnectionTo<Client>,
) -> Result<(), Error> {
    info!("Received cancel notification (v2): {notif:?}");
    todo!()
}
