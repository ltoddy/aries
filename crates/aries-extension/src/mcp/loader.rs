use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use aries_filesystem::walk::walk_dirs;

use crate::mcp::{McpConfig, McpLoadResult, McpServerConfig};

pub struct McpConfigLoader {
    roots: Vec<PathBuf>,
}

impl McpConfigLoader {
    pub const FILENAME: &str = "mcp.json";

    pub fn new(cwd: impl AsRef<Path>) -> Self {
        let cwd = cwd.as_ref();
        let home_dir = std::env::home_dir().unwrap_or_else(|| PathBuf::from("~"));

        let roots = vec![cwd.join(".agents").join("mcps"), home_dir.join(".agents").join("mcps")];

        Self { roots }
    }

    pub async fn load(&self) -> McpLoadResult<McpConfig> {
        let entries = walk_dirs(&self.roots, true, true)?;

        let file_paths = entries
            .into_iter()
            .filter(|entry| entry.is_file())
            .filter(|entry| entry.file_name().eq(&Some(OsStr::new(Self::FILENAME))))
            .collect::<Vec<_>>();

        let mut mcp_servers = HashMap::<String, McpServerConfig>::new();
        for file_path in file_paths {
            let content = tokio::fs::read_to_string(&file_path).await?;
            let Ok(config) = serde_json::from_str::<McpConfig>(&content) else { continue };
            for (name, server) in config.mcp_servers {
                mcp_servers.entry(name).or_insert(server);
            }
        }

        Ok(McpConfig { mcp_servers })
    }
}
