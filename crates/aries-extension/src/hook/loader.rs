use std::io;
use std::path::{Path, PathBuf};

use aries_filesystem::walk::walk_dirs;
use futures::stream::{self, StreamExt};

use crate::hook::preset::HooksPreset;

pub struct HooksLoader {
    roots: Vec<PathBuf>,
}

impl HooksLoader {
    pub const FILENAME: &str = "hooks.json";

    pub fn new(cwd: impl AsRef<Path>) -> Self {
        let cwd = cwd.as_ref();
        let home_dir = std::env::home_dir().unwrap_or_else(|| PathBuf::from("~"));

        let roots = vec![cwd.join(".agents").join("hooks"), home_dir.join(".agents").join("hooks")];

        Self { roots }
    }

    pub async fn load(&mut self) -> io::Result<Vec<HooksPreset>> {
        let entries = walk_dirs(&self.roots, true, false)?;

        let file_paths = entries
            .iter()
            .filter(|entry| entry.is_file() && entry.file_name().eq(&Some("hooks.json".as_ref())))
            .collect::<Vec<_>>();

        let presets = stream::iter(file_paths)
            .filter_map(|file_path| async move { HooksPreset::parse(file_path).await.ok() })
            .collect()
            .await;

        Ok(presets)
    }
}
