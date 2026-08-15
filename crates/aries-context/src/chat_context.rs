use std::io;
use std::path::Path;
use std::sync::Arc;

use aries_filesystem::jsonl;
use aries_filesystem::jsonl::JsonlAppender;
use rig_agent::completion::Message;
use tokio::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use tracing::error;

#[derive(Debug, Clone)]
pub struct ChatContext {
    inner: Arc<Inner>,
}

#[derive(Debug)]
struct Inner {
    history: RwLock<Vec<Message>>,
    file: JsonlAppender,
}

impl ChatContext {
    const FILENAME: &'static str = "chat-context.jsonl";

    pub async fn new(root_dir: impl AsRef<Path>) -> std::io::Result<Self> {
        let root_dir = root_dir.as_ref();
        let file_path = root_dir.join(Self::FILENAME);
        _ = tokio::fs::create_dir_all(root_dir).await;

        let history = Self::load(&file_path).await.unwrap_or_default();
        let file = JsonlAppender::open(&file_path).await?;

        let inner = Inner { history: RwLock::new(history), file };
        Ok(Self { inner: Arc::new(inner) })
    }

    pub async fn append(&self, messages: impl IntoIterator<Item = &Message>) {
        let messages = messages.into_iter().cloned().collect::<Vec<_>>();

        self.inner.history.write().await.extend(messages.clone());

        if let Err(err) = self.inner.file.append(&messages).await {
            error!(path = %self.inner.file.file_path().display(), err = %err, "failed to persist chat context");
        }
        let _ = self.inner.file.flush().await;
    }

    pub async fn overwrite(&self, messages: impl IntoIterator<Item = Message>) {
        let messages = messages.into_iter().collect::<Vec<_>>();

        *self.inner.history.write().await = messages.clone();

        if let Err(err) = self.inner.file.overwrite(&messages).await {
            error!(path = %self.inner.file.file_path().display(), err = %err, "failed to persist chat context");
        }
        let _ = self.inner.file.flush().await;
    }

    pub async fn history(&self) -> RwLockReadGuard<'_, Vec<Message>> {
        self.inner.history.read().await
    }

    pub async fn history_mut(&self) -> RwLockWriteGuard<'_, Vec<Message>> {
        self.inner.history.write().await
    }

    #[inline]
    async fn load(file_path: impl AsRef<Path>) -> io::Result<Vec<Message>> {
        jsonl::read(file_path).await
    }
}
