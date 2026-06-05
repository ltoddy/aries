use agent_client_protocol::schema::CancelNotification;
use agent_client_protocol::{Client, ConnectionTo, Error};
use tracing::info;

use crate::SharedRegistry;

pub async fn cancel(
    args: CancelNotification,
    _: ConnectionTo<Client>,
    registry: SharedRegistry,
) -> Result<(), Error> {
    info!("Received cancel notification {args:?}");

    let session_id = args.session_id.to_string();

    let session = {
        let reg = registry.lock().await;
        match reg.get_session(&session_id) {
            Some(session) => session,
            None => return Err(Error::resource_not_found(Some(session_id))),
        }
    };
    session.cancel();

    Ok(())
}
