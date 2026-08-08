use std::path::{Path, PathBuf};

use aries_filesystem::walk::walk_dirs;
use futures::stream::{self, StreamExt};
use itertools::Itertools;

use crate::hook::definition::HooksDefinition;

pub struct HooksLoader {
    roots: Vec<PathBuf>,
}

impl HooksLoader {
    pub const FILENAME: &str = "hooks.json";

    pub fn new(cwd: impl AsRef<Path>, home_dir: impl AsRef<Path>) -> Self {
        let cwd = cwd.as_ref();
        let home_dir = home_dir.as_ref();

        let roots = vec![
            home_dir.join(".agents").join("hooks"),
            home_dir.join(".agents").join("plugins").join("hooks"),
            cwd.join(".agents").join("hooks"),
        ]
        .into_iter()
        .unique()
        .collect_vec();

        Self { roots }
    }

    pub async fn load(&self) -> Vec<HooksDefinition> {
        let Ok(entries) = walk_dirs(&self.roots, true, false) else { return vec![] };

        let file_paths = entries
            .iter()
            .filter(|entry| entry.is_file() && entry.file_name().eq(&Some("hooks.json".as_ref())))
            .collect::<Vec<_>>();

        stream::iter(file_paths)
            .filter_map(|file_path| async move { HooksDefinition::parse(file_path).await.ok() })
            .collect()
            .await
    }
}
