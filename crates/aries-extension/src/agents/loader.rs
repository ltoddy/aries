use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use aries_filesystem::markdown::MarkdownFile;
use aries_filesystem::walk;
use futures::{StreamExt, stream};

use crate::agents::{CustomAgentDefinition, Frontmatter};

pub struct CustomAgentsLoader {
    roots: Vec<PathBuf>,
}

impl CustomAgentsLoader {
    pub fn new(cwd: impl AsRef<Path>) -> Self {
        let cwd = cwd.as_ref();
        let home_dir = std::env::home_dir().unwrap_or_else(|| PathBuf::from("~"));

        let roots =
            vec![home_dir.join(".agents").join("agents"), cwd.join("agents").join("agents")];

        Self { roots }
    }

    pub async fn load(&self) -> Vec<CustomAgentDefinition> {
        let Ok(file_paths) = walk::walk_dirs(&self.roots, false, true) else { return vec![] };

        let file_paths = file_paths
            .into_iter()
            .filter(|file_path| file_path.extension().eq(&Some(OsStr::new("md"))))
            .collect::<Vec<_>>();

        let agents = stream::iter(file_paths)
            .filter_map(|file_path| async move {
                let file = MarkdownFile::new(file_path);
                file.read::<Frontmatter>().await.ok()
            })
            .map(|file| CustomAgentDefinition::new(file.frontmatter, file.body))
            .collect::<Vec<_>>()
            .await;

        agents
    }
}
