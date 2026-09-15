use std::collections::HashMap;
use std::sync::Arc;

use agent_client_protocol::schema::v2::{
    ConnectMcpRequest, ConnectMcpResponse, DisconnectMcpRequest, DisconnectMcpResponse,
    McpConnectionId, McpServer, McpServerHttp, McpServerStdio, MessageMcpNotification,
    MessageMcpRequest, MessageMcpResponse,
};
use agent_client_protocol::{Client, Error, Responder, V2ConnectionTo};
use aries_extension::{McpDefinition, McpServerConfig};
use serde_json::value::RawValue;
use tracing::info;

#[derive(Debug, Clone)]
pub struct McpServers(pub Vec<McpServer>);

impl From<McpServers> for McpDefinition {
    fn from(val: McpServers) -> Self {
        let mut mcp_servers = HashMap::new();

        for server in val.0 {
            let (name, config) = match server {
                McpServer::Http(McpServerHttp { name, url, headers, .. }) => {
                    let headers = headers.into_iter().map(|v| (v.name, v.value)).collect();
                    (name, McpServerConfig::http(url, headers))
                },
                McpServer::Stdio(McpServerStdio { name, command, args, env, .. }) => {
                    let command = command.into_inner().display().to_string();
                    let env = env.into_iter().map(|v| (v.name, v.value)).collect();
                    (name, McpServerConfig::stdio(command, args, env))
                },
                _ => continue,
            };

            mcp_servers.entry(name).or_insert(config);
        }

        McpDefinition::new(mcp_servers)
    }
}

pub async fn connect(
    req: ConnectMcpRequest,
    responder: Responder<ConnectMcpResponse>,
    _cx: V2ConnectionTo<Client>,
) -> Result<(), Error> {
    info!("Received MCP connect request (v2): {req:?}");
    responder.respond(ConnectMcpResponse::new(McpConnectionId::new(req.server_id.to_string())))
}

pub async fn message(
    req: MessageMcpRequest,
    responder: Responder<MessageMcpResponse>,
    _cx: V2ConnectionTo<Client>,
) -> Result<(), Error> {
    info!("Received MCP message request (v2): {req:?}");
    let result = RawValue::from_string("{}".to_owned()).map_err(|_| Error::internal_error())?;
    responder.respond(MessageMcpResponse::new(Arc::from(result)))
}

pub async fn message_notification(
    notif: MessageMcpNotification,
    _cx: V2ConnectionTo<Client>,
) -> Result<(), Error> {
    info!("Received MCP message notification (v2): {notif:?}");
    Ok(())
}

pub async fn disconnect(
    req: DisconnectMcpRequest,
    responder: Responder<DisconnectMcpResponse>,
    _cx: V2ConnectionTo<Client>,
) -> Result<(), Error> {
    info!("Received MCP disconnect request (v2): {req:?}");
    responder.respond(DisconnectMcpResponse::new())
}
