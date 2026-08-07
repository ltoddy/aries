use std::path::{Path, PathBuf};

use aries_filesystem::document::FrontmatterDocument;
use aries_filesystem::walk;
use futures::{StreamExt, stream};
use itertools::Itertools;

use crate::agent::AgentDefinition;

pub struct AgentsLoader {
    roots: Vec<PathBuf>,
}

impl AgentsLoader {
    pub fn new(cwd: impl AsRef<Path>, home_dir: impl AsRef<Path>) -> Self {
        let cwd = cwd.as_ref();
        let home_dir = home_dir.as_ref();

        let roots = vec![
            home_dir.join(".agents").join("agents"),
            home_dir.join(".agents").join("plugins").join("agents"),
            cwd.join(".agents").join("agents"),
        ]
        .into_iter()
        .unique()
        .collect_vec();

        Self { roots }
    }

    pub async fn load(&self) -> Vec<AgentDefinition> {
        let Ok(file_paths) = walk::walk_dirs(&self.roots, false, true) else { return vec![] };

        let file_paths = file_paths
            .into_iter()
            .filter(|file_path| file_path.extension().eq(&Some("md".as_ref())))
            .collect::<Vec<_>>();

        let agents = stream::iter(file_paths)
            .filter_map(|file_path| async move { FrontmatterDocument::read(file_path).await.ok() })
            .map(|doc| AgentDefinition::new(doc.location, doc.frontmatter, doc.body))
            .collect::<Vec<_>>()
            .await;

        agents
            .into_iter()
            .unique_by(|a| a.frontmatter.name.clone())
            .collect::<Vec<AgentDefinition>>()
    }
}
