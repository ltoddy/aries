mod definition;
mod error;
mod loader;
#[cfg(test)]
mod tests;
mod tool;

use std::collections::HashMap;

use futures::future;
use http::{HeaderName, HeaderValue};
use rig_agent::tool::rmcp::McpClientHandler;
use rig_agent::tool::server::ToolServerHandle;
use rmcp::RoleClient;
use rmcp::model::{ClientCapabilities, ClientInfo, Implementation};
use rmcp::service::RunningService;
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
use rmcp::transport::{ConfigureCommandExt, StreamableHttpClientTransport, TokioChildProcess};
use tracing::{info, warn};

pub use self::definition::{Http, McpDefinition, McpServerConfig, Sse, Stdio};
pub use self::error::{McpConnectError, McpParseError};
pub use self::loader::McpsLoader;

pub type McpLoadResult<T> = Result<T, McpParseError>;
pub type McpConnectResult<T> = Result<T, McpConnectError>;

const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

pub async fn connect(
    mcps: &[McpDefinition],
    tool_server_handle: ToolServerHandle,
) -> Vec<RunningService<RoleClient, McpClientHandler>> {
    let mcp_servers = mcps.iter().flat_map(|c| &c.mcp_servers).collect::<Vec<_>>();

    future::join_all(mcp_servers.into_iter().map(|(server_name, server_config)| {
        connect_one(server_name, server_config, tool_server_handle.clone())
    }))
    .await
    .into_iter()
    .flatten()
    .collect()
}

async fn connect_one(
    server_name: impl Into<String>,
    config: &McpServerConfig,
    tool_server_handle: ToolServerHandle,
) -> McpConnectResult<RunningService<RoleClient, McpClientHandler>> {
    let server_name = server_name.into();
    info!(server = %server_name, "Connecting to mcp server");

    let client_info = ClientInfo::new(ClientCapabilities::default(), Implementation::default());
    let handler = McpClientHandler::new(client_info.clone(), tool_server_handle);

    let service = match config {
        McpServerConfig::Stdio(Stdio { command, args, env }) => {
            let cmd = tokio::process::Command::new(command).configure(|cmd| {
                cmd.args(args).envs(env);
            });

            let child = TokioChildProcess::new(cmd).map_err(McpConnectError::spawn)?;
            tokio::time::timeout(TIMEOUT, handler.connect(child))
                .await
                .map_err(|_| McpConnectError::timeout(TIMEOUT))?
        },
        McpServerConfig::Sse(Sse { url, headers })
        | McpServerConfig::Http(Http { url, headers }) => {
            let custom_headers: HashMap<HeaderName, HeaderValue> = headers
                .iter()
                .filter_map(|(k, v)| Some((k.parse().ok()?, v.parse().ok()?)))
                .collect::<HashMap<_, _>>();
            let config = StreamableHttpClientTransportConfig::with_uri(url.to_owned())
                .custom_headers(custom_headers);
            let transport = StreamableHttpClientTransport::from_config(config);
            tokio::time::timeout(TIMEOUT, handler.connect(transport))
                .await
                .map_err(|_| McpConnectError::timeout(TIMEOUT))?
        },
    };

    match service {
        Ok(service) => {
            info!("Connected to mcp server: {:?}", service.peer_info());
            Ok(service)
        },
        Err(err) => {
            warn!(error = ?err, "Failed to connect to mcp server");
            Err(McpConnectError::client(err))
        },
    }
}
