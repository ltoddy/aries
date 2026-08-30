use std::collections::HashMap;

use agent_client_protocol::schema::v1::{McpServer, McpServerHttp, McpServerSse, McpServerStdio};
use aries_extension::{McpDefinition, McpServerConfig};

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
                McpServer::Sse(McpServerSse { name, url, headers, .. }) => {
                    let headers = headers.into_iter().map(|v| (v.name, v.value)).collect();
                    (name, McpServerConfig::sse(url, headers))
                },
                McpServer::Stdio(McpServerStdio { name, command, args, env, .. }) => {
                    let command = command.display().to_string();
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
