pub mod config;
pub mod error;
pub mod loader;
pub mod tool;

use std::collections::HashMap;

use http::{HeaderName, HeaderValue};
use rig_core::tool::ToolDyn;
use rig_core::tool::rmcp::McpTool;
use rmcp::model::ClientInfo;
use rmcp::service::RunningService;
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
use rmcp::transport::{ConfigureCommandExt, StreamableHttpClientTransport, TokioChildProcess};
use rmcp::{RoleClient, ServiceExt};
use tracing::info;

pub use self::config::{HttpConfig, McpConfig, McpServerConfig, SseConfig, StdioConfig};
pub use self::error::{McpConnectError, McpParseError};
pub use self::loader::McpConfigLoader;
pub use self::tool::NamespacedMcpTool;

pub type McpLoadResult<T> = Result<T, McpParseError>;
pub type McpConnectResult<T> = Result<T, McpConnectError>;

pub async fn connect(
    config: McpConfig,
) -> (Vec<RunningService<RoleClient, ClientInfo>>, Vec<Box<dyn ToolDyn>>) {
    let mut tools = Vec::<Box<dyn ToolDyn>>::new();
    let mut services = Vec::<RunningService<RoleClient, ClientInfo>>::new();

    for (server_name, server_config) in config.mcp_servers {
        match connect_one(&server_name, server_config).await {
            Ok((service, server_tools)) => {
                tools.extend(server_tools);
                services.push(service);
            },
            Err(err) => {
                tracing::warn!(server = %server_name, error = %err, "failed to connect to MCP server, skipping");
            },
        }
    }

    (services, tools)
}

async fn connect_one(
    server_name: impl Into<String>,
    config: McpServerConfig,
) -> McpConnectResult<(RunningService<RoleClient, ClientInfo>, Vec<Box<dyn ToolDyn>>)> {
    let server_name = server_name.into();
    let client_info = ClientInfo::default();

    info!(server = %server_name, "connecting to MCP server");

    let service = match config {
        McpServerConfig::Stdio(StdioConfig { command, args, env }) => {
            let cmd = tokio::process::Command::new(command).configure(|cmd| {
                cmd.args(&args).envs(&env);
            });

            let child = TokioChildProcess::new(cmd)
                .map_err(|e| McpConnectError::Connection(e.to_string()))?;
            client_info
                .serve(child)
                .await
                .map_err(|e| McpConnectError::Connection(e.to_string()))?
        },
        McpServerConfig::Sse(SseConfig { url, headers }) => {
            let custom_headers: HashMap<HeaderName, HeaderValue> = headers
                .into_iter()
                .filter_map(|(k, v)| Some((k.parse().ok()?, v.parse().ok()?)))
                .collect::<HashMap<_, _>>();
            let config =
                StreamableHttpClientTransportConfig::with_uri(url).custom_headers(custom_headers);
            let transport = StreamableHttpClientTransport::from_config(config);
            client_info
                .serve(transport)
                .await
                .map_err(|e| McpConnectError::Connection(e.to_string()))?
        },
        McpServerConfig::Http(HttpConfig { url, headers }) => {
            let custom_headers: HashMap<HeaderName, HeaderValue> = headers
                .into_iter()
                .filter_map(|(k, v)| Some((k.parse().ok()?, v.parse().ok()?)))
                .collect::<HashMap<_, _>>();
            let config =
                StreamableHttpClientTransportConfig::with_uri(url).custom_headers(custom_headers);
            let transport = StreamableHttpClientTransport::from_config(config);
            client_info
                .serve(transport)
                .await
                .map_err(|e| McpConnectError::Connection(e.to_string()))?
        },
    };

    let peer = service.peer();
    let tools = peer.list_all_tools().await?;
    let tools = tools
        .into_iter()
        .map(|tool| {
            let tool = McpTool::from_mcp_server(tool, peer.clone());
            Box::new(NamespacedMcpTool::new(&server_name, tool)) as Box<dyn ToolDyn>
        })
        .collect::<Vec<_>>();

    tracing::info!(server = %server_name, tool_count = tools.len(), "connected to MCP server");

    Ok((service, tools))
}
