mod error;
mod file_checkpoint;
mod read_state;
mod task;

use std::path::Path;
use std::time::UNIX_EPOCH;

use aries_event::Notifier;
use aries_lspclient::SharedLspClient;
use tokio::fs;

pub use self::error::GuardWriteError;
pub use self::file_checkpoint::SharedFileCheckpoint;
pub use self::read_state::SharedReadState;
pub use self::task::{StopTaskError, TaskKind, TaskRegistry, TaskSnapshot, TaskStatus};

#[derive(Clone)]
pub struct ToolContext {
    lsp_client: Option<SharedLspClient>,
    pub file_checkpoint: SharedFileCheckpoint,
    pub task: TaskRegistry,
    read_state: SharedReadState,
}

impl std::fmt::Debug for ToolContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolContext")
            .field("has_lsp_client", &self.lsp_client.is_some())
            .field("file_checkpoint", &self.file_checkpoint)
            .field("task", &self.task)
            .field("read_state", &self.read_state)
            .finish()
    }
}

impl ToolContext {
    pub fn new(lsp_client: Option<SharedLspClient>, notifier: Notifier) -> Self {
        let file_checkpoint = SharedFileCheckpoint::new();
        let task = TaskRegistry::new(notifier);
        let read_state = SharedReadState::new();

        Self { lsp_client, file_checkpoint, task, read_state }
    }

    pub async fn on_file_read(&self, file_path: impl AsRef<Path>) {
        let file_path = file_path.as_ref();

        let Some(modified) = Self::modified_at(file_path).await else { return };

        let file_path =
            fs::canonicalize(file_path).await.unwrap_or_else(|_| file_path.to_path_buf());

        self.read_state.record(file_path, modified);
    }

    pub async fn guard_write(&self, file_path: impl AsRef<Path>) -> Result<(), GuardWriteError> {
        let file_path = file_path.as_ref();
        let file_path =
            fs::canonicalize(file_path).await.unwrap_or_else(|_| file_path.to_path_buf());

        let Some(record) = self.read_state.get(&file_path) else {
            return Err(GuardWriteError::NotRead);
        };

        if let Some(current) = Self::modified_at(file_path).await
            && current > record.timestamp_millis
        {
            return Err(GuardWriteError::ModifiedSinceRead);
        }
        Ok(())
    }

    pub async fn on_file_written(&self, file_path: impl AsRef<Path>, content: impl Into<String>) {
        let file_path = file_path.as_ref();
        let content = content.into();

        if let Some(client) = &self.lsp_client {
            let _ = client.did_change(file_path, &content).await;
            let _ = client.did_save(file_path, &content).await;
        }

        self.on_file_read(file_path).await;
    }

    async fn modified_at(file_path: impl AsRef<Path>) -> Option<u128> {
        let modified =
            fs::metadata(file_path).await.and_then(|metadata| metadata.modified()).ok()?;

        let since = modified.duration_since(UNIX_EPOCH).ok()?;
        Some(since.as_millis())
    }
}
