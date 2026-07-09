use agent_client_protocol::schema::v2::CancelSessionNotification;
use agent_client_protocol::{Client, ConnectionTo, Error};
use tracing::info;

pub async fn cancel(
    notif: CancelSessionNotification,
    _cx: ConnectionTo<Client>,
) -> Result<(), Error> {
    info!("Received cancel notification (v2): {notif:?}");
    todo!()
}
