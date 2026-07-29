use agent_client_protocol::schema::v1::CancelNotification;
use agent_client_protocol::{Client, ConnectionTo, Error};
use tracing::{info, instrument};

use super::SharedRegistry;

#[instrument(name = "acp.cancel", skip_all, fields(session_id = %args.session_id))]
pub async fn cancel(
    args: CancelNotification,
    _: ConnectionTo<Client>,
    registry: SharedRegistry,
) -> Result<(), Error> {
    info!("Received cancel notification {args:?}");

    let session_id = args.session_id.to_string();

    let session = {
        let registry = registry.lock().await;
        match registry.get_session(&session_id) {
            Some(session) => session,
            None => return Err(Error::resource_not_found(Some(session_id))),
        }
    };
    session.cancel();

    Ok(())
}
