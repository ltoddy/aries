use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use aries_filesystem::walk::walk_dirs;
use futures::stream::{self, StreamExt};
use itertools::Itertools;

use crate::mcp::definition::McpDefinition;

#[derive(Debug)]
pub struct McpsLoader {
    roots: Vec<PathBuf>,
}

impl McpsLoader {
    pub const FILENAME: &str = "mcp.json";

    pub fn new(cwd: impl AsRef<Path>, home_dir: impl AsRef<Path>) -> Self {
        let cwd = cwd.as_ref();
        let home_dir = home_dir.as_ref();

        let roots = vec![
            home_dir.join(".agents").join("mcps"),
            home_dir.join(".agents").join("plugins").join("mcps"),
            cwd.join(".agents").join("mcps"),
        ]
        .into_iter()
        .unique()
        .collect_vec();

        Self { roots }
    }

    pub async fn load(&self) -> Vec<McpDefinition> {
        let Ok(entries) = walk_dirs(&self.roots, true, true) else { return vec![] };

        let file_paths = entries
            .into_iter()
            .filter(|entry| entry.file_name().eq(&Some(OsStr::new(Self::FILENAME))))
            .collect::<Vec<_>>();

        stream::iter(file_paths)
            .filter_map(|file_path| async move { McpDefinition::parse(file_path).await.ok() })
            .collect::<Vec<_>>()
            .await
    }
}
