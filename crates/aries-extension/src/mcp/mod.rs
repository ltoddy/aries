mod definition;
mod error;
mod loader;
mod tool;

use std::collections::HashMap;

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

pub async fn connect(
    mcps: &[McpDefinition],
    tool_server_handle: ToolServerHandle,
) -> Vec<RunningService<RoleClient, McpClientHandler>> {
    let mcp_servers = mcps.iter().flat_map(|c| &c.mcp_servers).collect::<Vec<_>>();

    let mut services = Vec::with_capacity(mcp_servers.len());
    for (server_name, server_config) in mcp_servers {
        info!(server = %server_name, "Connecting to mcp server");
        match connect_one(server_config, tool_server_handle.clone()).await {
            Ok(service) => {
                info!("Connected to mcp server: {:?}", service.peer_info());
                services.push(service);
            },
            Err(err) => warn!(error = ?err, "Failed to connect to mcp server"),
        }
    }

    services
}

async fn connect_one(
    config: &McpServerConfig,
    tool_server_handle: ToolServerHandle,
) -> McpConnectResult<RunningService<RoleClient, McpClientHandler>> {
    let client_info = ClientInfo::new(ClientCapabilities::default(), Implementation::default());
    let handler = McpClientHandler::new(client_info.clone(), tool_server_handle);

    let mcp_service = match config {
        McpServerConfig::Stdio(Stdio { command, args, env }) => {
            let cmd = tokio::process::Command::new(command).configure(|cmd| {
                cmd.args(args).envs(env);
            });

            let child = TokioChildProcess::new(cmd)
                .map_err(|e| McpConnectError::Connection(e.to_string()))?;
            handler.connect(child).await.map_err(|e| McpConnectError::Connection(e.to_string()))?
        },
        McpServerConfig::Sse(Sse { url, headers }) => {
            let custom_headers: HashMap<HeaderName, HeaderValue> = headers
                .iter()
                .filter_map(|(k, v)| Some((k.parse().ok()?, v.parse().ok()?)))
                .collect::<HashMap<_, _>>();
            let config = StreamableHttpClientTransportConfig::with_uri(url.to_owned())
                .custom_headers(custom_headers);
            let transport = StreamableHttpClientTransport::from_config(config);
            handler
                .connect(transport)
                .await
                .map_err(|e| McpConnectError::Connection(e.to_string()))?
        },
        McpServerConfig::Http(Http { url, headers }) => {
            let custom_headers: HashMap<HeaderName, HeaderValue> = headers
                .iter()
                .filter_map(|(k, v)| Some((k.parse().ok()?, v.parse().ok()?)))
                .collect::<HashMap<_, _>>();
            let config = StreamableHttpClientTransportConfig::with_uri(url.to_owned())
                .custom_headers(custom_headers);
            let transport = StreamableHttpClientTransport::from_config(config);
            handler
                .connect(transport)
                .await
                .map_err(|e| McpConnectError::Connection(e.to_string()))?
        },
    };

    Ok(mcp_service)
}
