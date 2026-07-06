use std::collections::HashMap;

use agent_client_protocol::schema::v1::McpServer;
use aries_extension::mcp::{HttpConfig, McpConfig, McpServerConfig, SseConfig, StdioConfig};

pub fn convert_acp_mcp_servers(servers: Vec<McpServer>) -> McpConfig {
    let mut mcp_servers = HashMap::new();

    for server in servers {
        let (name, config) = match server {
            McpServer::Stdio(s) => {
                let env = s.env.into_iter().map(|e| (e.name, e.value)).collect::<HashMap<_, _>>();
                (
                    s.name,
                    McpServerConfig::Stdio(StdioConfig {
                        command: s.command.display().to_string(),
                        args: s.args,
                        env,
                    }),
                )
            },
            McpServer::Http(h) => (
                h.name,
                McpServerConfig::Http(HttpConfig {
                    url: h.url,
                    headers: h.headers.into_iter().map(|hdr| (hdr.name, hdr.value)).collect(),
                }),
            ),
            McpServer::Sse(s) => (
                s.name,
                McpServerConfig::Sse(SseConfig {
                    url: s.url,
                    headers: s.headers.into_iter().map(|hdr| (hdr.name, hdr.value)).collect(),
                }),
            ),
            _ => continue,
        };

        mcp_servers.entry(name).or_insert(config);
    }

    McpConfig { mcp_servers }
}
