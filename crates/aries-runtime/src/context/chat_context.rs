use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rig::completion::Message;
use tokio::sync::RwLock;
use tracing::error;

#[derive(Debug, Clone)]
pub struct ChatContext {
    history: Arc<RwLock<Vec<Message>>>,
    file_path: PathBuf,
}

impl ChatContext {
    const FILENAME: &'static str = "chat-context.jsonl";

    pub async fn new(root_dir: impl AsRef<Path>) -> io::Result<Self> {
        let root_dir = root_dir.as_ref();
        let file_path = root_dir.join(Self::FILENAME);
        if !root_dir.exists() {
            tokio::fs::create_dir_all(root_dir).await?;
        }

        tokio::fs::OpenOptions::new().append(true).create(true).open(&file_path).await?;
        let history = Arc::new(RwLock::new(
            tokio_jsonl::read_all(&file_path).await.map_err(io::Error::other)?,
        ));

        Ok(Self { history, file_path })
    }

    pub async fn append<'a>(&self, messages: impl IntoIterator<Item = &'a Message>) {
        let messages = messages.into_iter().collect::<Vec<_>>();
        let mut history = self.history.write().await;
        history.extend(messages.iter().copied().cloned());

        if let Err(err) = tokio_jsonl::append_all(&self.file_path, &messages).await {
            error!(path = %self.file_path.display(), err = %err, "failed to persist chat context");
        }
    }

    pub async fn overwrite(&self, messages: impl IntoIterator<Item = Message>) {
        let mut history = self.history.write().await;
        *history = messages.into_iter().collect();

        if let Err(err) = tokio_jsonl::write_all(&self.file_path, history.iter()).await {
            error!(path = %self.file_path.display(), err = %err, "failed to persist chat context");
        }
    }

    pub async fn history(&self) -> Vec<Message> {
        self.history.read().await.clone()
    }
}
