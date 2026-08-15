use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use aries_filesystem::jsonl;
use itertools::Itertools;
use parking_lot::RwLock;
use rig_agent::completion::Message;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use tracing::{Instrument, Span, error};

#[derive(Clone)]
pub struct ChatContext {
    inner: Arc<Inner>,
}

struct Inner {
    history: RwLock<Vec<Message>>,
    sender: UnboundedSender<Vec<Message>>,
    file_path: PathBuf,
}

impl ChatContext {
    const FILENAME: &'static str = "chat-context.jsonl";

    pub async fn new(root_dir: impl AsRef<Path>) -> Self {
        let root_dir = root_dir.as_ref();
        let file_path = root_dir.join(Self::FILENAME);

        let history = Self::load(&file_path).await.unwrap_or_default();

        let (sender, receiver) = unbounded_channel();
        tokio::spawn(refresh_context(receiver, file_path.clone()).instrument(Span::current()));

        let inner = Inner { history: RwLock::new(history), sender, file_path };
        Self { inner: Arc::new(inner) }
    }

    pub fn extend(&self, messages: impl IntoIterator<Item = Message>) {
        self.inner.history.write().extend(messages);

        let snapshot = self.inner.history.read().clone();
        if let Err(err) = self.inner.sender.send(snapshot) {
            error!(err = %err, "failed to send context history for persistence")
        }
    }

    pub async fn overwrite(&self, messages: impl IntoIterator<Item = Message>) {
        *self.inner.history.write() = messages.into_iter().collect_vec();

        let snapshot = self.inner.history.read().clone();
        if let Err(err) = jsonl::write(&self.inner.file_path, &snapshot).await {
            error!("failed to write context history to {}: {err}", self.inner.file_path.display());
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

async fn refresh_context(mut rx: UnboundedReceiver<Vec<Message>>, file_path: impl AsRef<Path>) {
    let file_path = file_path.as_ref();

    while let Some(messages) = rx.recv().await {
        if let Some(parent) = file_path.parent() {
            _ = tokio::fs::create_dir_all(parent).await;
        }

        if let Err(err) = jsonl::write(file_path, &messages).await {
            error!("failed to write context history to {}: {err}", file_path.display());
        }
    }
}
