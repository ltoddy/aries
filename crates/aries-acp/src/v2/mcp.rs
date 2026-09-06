use agent_client_protocol::schema::v2::{
    ConnectMcpRequest, ConnectMcpResponse, DisconnectMcpRequest, DisconnectMcpResponse,
    MessageMcpNotification, MessageMcpRequest, MessageMcpResponse,
};
use agent_client_protocol::{Client, Error, Responder, V2ConnectionTo};
use tracing::info;

pub async fn connect(
    req: ConnectMcpRequest,
    _responder: Responder<ConnectMcpResponse>,
    _cx: V2ConnectionTo<Client>,
) -> Result<(), Error> {
    info!("Received MCP connect request (v2): {req:?}");
    todo!()
}

pub async fn message(
    req: MessageMcpRequest,
    _responder: Responder<MessageMcpResponse>,
    _cx: V2ConnectionTo<Client>,
) -> Result<(), Error> {
    info!("Received MCP message request (v2): {req:?}");
    todo!()
}

pub async fn message_notification(
    notif: MessageMcpNotification,
    _cx: V2ConnectionTo<Client>,
) -> Result<(), Error> {
    info!("Received MCP message notification (v2): {notif:?}");
    todo!()
}

pub async fn disconnect(
    req: DisconnectMcpRequest,
    _responder: Responder<DisconnectMcpResponse>,
    _cx: V2ConnectionTo<Client>,
) -> Result<(), Error> {
    info!("Received MCP disconnect request (v2): {req:?}");
    todo!()
}
