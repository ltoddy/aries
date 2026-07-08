use agent_client_protocol::schema::v2::CancelNotification;
use agent_client_protocol::{Client, ConnectionTo, Error};
use tracing::info;

pub async fn cancel(notif: CancelNotification, _cx: ConnectionTo<Client>) -> Result<(), Error> {
    info!("Received cancel notification (v2): {notif:?}");
    todo!()
}
