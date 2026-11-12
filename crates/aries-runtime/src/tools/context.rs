use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::UNIX_EPOCH;

use aries_communicate::Notifier;
use tokio::fs;

use crate::tools::read_state::SharedReadState;
use crate::tools::task::TaskRegistry;
use crate::tools::worktree::Worktree;

#[derive(Clone)]
pub struct ToolContext {
    session_id: String,
    cwd: Arc<parking_lot::Mutex<PathBuf>>,
    worktree: Arc<parking_lot::Mutex<Option<Worktree>>>,
    pub task: TaskRegistry,
    read_state: SharedReadState,
}

impl std::fmt::Debug for ToolContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolContext")
            .field("cwd", &self.cwd)
            .field("task", &self.task)
            .field("read_state", &self.read_state)
            .finish()
    }
}

#[derive(thiserror::Error, Debug)]
pub enum GuardWriteError {
    #[error("file has not been read yet. read it first before writing to it")]
    NotRead,

    #[error(
        "file has been modified since read, either by the user or by a linter. read it again before attempting to write it"
    )]
    ModifiedSinceRead,
}

impl ToolContext {
    pub fn new(session_id: impl Into<String>, notifier: Notifier, cwd: impl Into<PathBuf>) -> Self {
        let session_id = session_id.into();
        let task = TaskRegistry::new(notifier);
        let read_state = SharedReadState::new();
        let cwd = std::sync::Arc::new(parking_lot::Mutex::new(cwd.into()));
        let worktree_session = std::sync::Arc::new(parking_lot::Mutex::new(None));

        Self { session_id, cwd, worktree: worktree_session, task, read_state }
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn cwd(&self) -> PathBuf {
        self.cwd.lock().clone()
    }

    pub fn set_cwd(&self, cwd: impl Into<PathBuf>) {
        *self.cwd.lock() = cwd.into();
    }

    pub fn worktree(&self) -> Option<Worktree> {
        self.worktree.lock().clone()
    }

    pub fn set_worktree(&self, worktree: Option<Worktree>) {
        *self.worktree.lock() = worktree;
    }

    pub async fn on_file_read(&self, file_path: impl AsRef<Path>) {
        let file_path = file_path.as_ref();

        let Some(modified) = Self::modified_at(file_path).await else { return };

        let file_path = fs::canonicalize(file_path).await.unwrap_or_else(|_| file_path.to_owned());

        self.read_state.record(file_path, modified);
    }

    pub async fn guard_write(&self, file_path: impl AsRef<Path>) -> Result<(), GuardWriteError> {
        let file_path = file_path.as_ref();
        let file_path = fs::canonicalize(file_path).await.unwrap_or_else(|_| file_path.to_owned());

        let Some(record) = self.read_state.get(&file_path) else {
            return Err(GuardWriteError::NotRead);
        };

        if let Some(current) = Self::modified_at(&file_path).await
            && current > record.timestamp_millis
        {
            return Err(GuardWriteError::ModifiedSinceRead);
        }
        Ok(())
    }

    async fn modified_at(file_path: impl AsRef<Path>) -> Option<u128> {
        let modified =
            fs::metadata(file_path).await.and_then(|metadata| metadata.modified()).ok()?;

        let since = modified.duration_since(UNIX_EPOCH).ok()?;
        Some(since.as_millis())
    }
}
