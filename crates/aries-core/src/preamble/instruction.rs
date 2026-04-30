use std::path::{Path, PathBuf};

pub struct AgentsmdFileLoader {
    file_path: PathBuf,
}

impl AgentsmdFileLoader {
    const FILENAME: &str = "AGENTS.md";

    pub fn new(dir: impl AsRef<Path>) -> Self {
        let file_path = dir.as_ref().join(Self::FILENAME);

        Self { file_path }
    }

    pub async fn read(&self) -> Option<String> {
        if !self.file_path.exists() {
            return None;
        }

        tokio::fs::read_to_string(&self.file_path).await.ok()
    }

    pub fn file_path(&self) -> &Path {
        &self.file_path
    }
}
