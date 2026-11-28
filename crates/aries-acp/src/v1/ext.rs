use agent_client_protocol::v1::{ConnectionTo, Responder};
use agent_client_protocol::{Client, Error, Handled, UntypedMessage};
use tracing::info;

pub async fn message(
    req: UntypedMessage,
    responder: Responder<serde_json::Value>,
    _: ConnectionTo<Client>,
) -> Result<Handled<(UntypedMessage, Responder<serde_json::Value>)>, Error> {
    if !req.method.starts_with('_') {
        return Ok(Handled::No { message: (req, responder), retry: false });
    }

    info!("Received extension request {} (v1)", req.method);

    let method = req.method;
    responder.respond_with_error(Error::method_not_found().data(method))?;

    Ok(Handled::Yes)
}

pub async fn message_notification(
    notif: UntypedMessage,
    cx: ConnectionTo<Client>,
) -> Result<Handled<(UntypedMessage, ConnectionTo<Client>)>, Error> {
    if !notif.method.starts_with('_') {
        return Ok(Handled::No { message: (notif, cx), retry: false });
    }

    info!("Received extension notification {} (v1)", notif.method);

    Ok(Handled::Yes)
}
