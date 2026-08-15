use std::io;
use std::path::Path;
use std::sync::Arc;

use aries_filesystem::jsonl;
use aries_filesystem::jsonl::JsonlAppender;
use itertools::Itertools;
use parking_lot::RwLock;
use rig_agent::completion::Message;
use tracing::error;

#[derive(Clone)]
pub struct ChatContext {
    inner: Arc<Inner>,
}

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
        if let Err(err) = self.inner.file.append(messages).await {
            error!(path = %self.inner.file.file_path().display(), err = %err, "failed to append chat context");
        }
    }

    pub async fn overwrite(&self, messages: impl IntoIterator<Item = Message>) {
        *self.inner.history.write() = messages.into_iter().collect_vec();

        let snapshot = self.inner.history.read().clone();
        if let Err(err) = self.inner.file.overwrite(&snapshot).await {
            error!(path = %self.inner.file.file_path().display(), err = %err, "failed to overwrite chat context");
        }
    }

    pub fn history(&self) -> parking_lot::RwLockReadGuard<'_, Vec<Message>> {
        self.inner.history.read()
    }

    pub fn history_mut(&self) -> parking_lot::RwLockWriteGuard<'_, Vec<Message>> {
        self.inner.history.write()
    }

    #[inline]
    async fn load(file_path: impl AsRef<Path>) -> io::Result<Vec<Message>> {
        jsonl::read(file_path).await
    }
}
