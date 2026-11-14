use agent_client_protocol::v1::ConnectionTo;
use agent_client_protocol::v1::schema::{self, CancelNotification, CancelRequestNotification};
use agent_client_protocol::{Client, Error};
use aries_runtime::registry::SharedRegistry;
use tracing::{info, instrument};

pub async fn cancel_request(
    notif: CancelRequestNotification,
    _: ConnectionTo<Client>,
) -> Result<(), Error> {
    info!("Received cancel request notification (v1): {notif:?}");

    // TODO: Cancel the request identified by notif.request_id.
    Ok(())
}

#[instrument(name = "acp.cancel", skip_all, fields(session_id = %args.session_id))]
pub async fn cancel(
    args: CancelNotification,
    _: ConnectionTo<Client>,
    registry: &SharedRegistry,
) -> Result<(), Error> {
    info!("Received cancel notification {args:?}");

    let session_id = args.session_id.to_string();

    let session = {
        let registry = registry.lock().await;
        match registry.session(&session_id) {
            Some(session) => session,
            None => return Err(schema::Error::resource_not_found(Some(session_id)).into()),
        }
    };
    session.cancel();

    Ok(())
}
