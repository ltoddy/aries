use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use parking_lot::Mutex;

// TODO: FileCheckpoint 可能需要增加 file watch 能力 ?

#[derive(Clone, Debug)]
pub struct SharedFileCheckpoint(Arc<Mutex<FileCheckpoint>>);

impl Default for SharedFileCheckpoint {
    fn default() -> Self {
        Self::new()
    }
}

impl SharedFileCheckpoint {
    const MB: usize = 1024 * 1024; // 超过 1 MB 什么也不做, 避免占用内存

    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(FileCheckpoint::new())))
    }

    pub async fn push(
        &self,
        file_path: impl AsRef<Path>,
        content: impl Into<String>,
    ) -> std::io::Result<()> {
        let content = content.into();
        if content.len() > Self::MB {
            return Ok(());
        }

        let file_path = tokio::fs::canonicalize(file_path).await?;

        let mut guard = self.0.lock();
        guard.backups.insert(file_path, content);

        Ok(())
    }

    pub async fn pop(&self, file_path: impl AsRef<Path>) -> std::io::Result<Option<String>> {
        let file_path = tokio::fs::canonicalize(file_path).await?;

        let mut guard = self.0.lock();
        Ok(guard.backups.remove(&file_path))
    }
}

#[derive(Clone, Debug)]
struct FileCheckpoint {
    backups: HashMap<PathBuf, String>,
}

impl FileCheckpoint {
    fn new() -> Self {
        Self { backups: HashMap::with_capacity(32) }
    }
}
