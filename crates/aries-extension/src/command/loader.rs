use std::path::{Path, PathBuf};

use aries_filesystem::document::FrontmatterDocument;
use aries_filesystem::walk;
use futures::{StreamExt, stream};
use itertools::Itertools;

use crate::command::definition::CommandDefinition;

#[derive(Debug)]
pub struct CommandsLoader {
    roots: Vec<PathBuf>,
}

impl CommandsLoader {
    pub fn new(cwd: impl AsRef<Path>, home_dir: impl AsRef<Path>) -> Self {
        let cwd = cwd.as_ref();
        let home_dir = home_dir.as_ref();

        let roots =
            vec![home_dir.join(".agents").join("commands"), cwd.join(".agents").join("commands")]
                .into_iter()
                .unique()
                .collect_vec();

        Self { roots }
    }

    pub async fn load(&self) -> Vec<CommandDefinition> {
        let Ok(file_paths) = walk::walk_dirs(&self.roots, false, true) else { return vec![] };

        let file_paths = file_paths
            .into_iter()
            .filter(|file_path| file_path.extension().eq(&Some("md".as_ref())))
            .collect_vec();

        stream::iter(file_paths)
            .filter_map(|file_path| async move { FrontmatterDocument::read(file_path).await.ok() })
            .map(|doc| CommandDefinition::new(doc.location, doc.frontmatter, doc.body))
            .collect::<Vec<_>>()
            .await
    }
}
