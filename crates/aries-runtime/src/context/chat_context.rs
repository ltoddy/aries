use std::io;
use std::path::Path;
use std::sync::Arc;

use rig::completion::Message;
use tokio::sync::RwLock;
use tracing::error;

use crate::jsonl::JsonlAppender;

#[derive(Debug, Clone)]
pub struct ChatContext {
    history: Arc<RwLock<Vec<Message>>>,
    file: JsonlAppender,
}

impl ChatContext {
    const FILENAME: &'static str = "chat-context.jsonl";

    pub async fn new(root_dir: impl AsRef<Path>) -> io::Result<Self> {
        let root_dir = root_dir.as_ref();
        let file_path = root_dir.join(Self::FILENAME);
        if !root_dir.exists() {
            tokio::fs::create_dir_all(root_dir).await?;
        }

        let file = JsonlAppender::open(file_path).await?;
        let history = Arc::new(RwLock::new(file.read().await?));

        Ok(Self { history, file })
    }

    pub async fn append<'a>(&self, messages: impl IntoIterator<Item = &'a Message>) {
        let messages = messages.into_iter().collect::<Vec<_>>();
        let mut history = self.history.write().await;
        history.extend(messages.iter().copied().cloned());

        if let Err(err) = self.file.append(messages).await {
            error!(path = %self.file.file_path().display(), err = %err, "failed to persist chat context");
        }
        if let Err(err) = self.file.flush().await {
            error!(path = %self.file.file_path().display(), err = %err, "failed to flush chat context");
        }
    }

    pub async fn overwrite(&self, messages: impl IntoIterator<Item = Message>) {
        let mut history = self.history.write().await;
        *history = messages.into_iter().collect();

        if let Err(err) = self.file.overwrite(&*history).await {
            error!(path = %self.file.file_path().display(), err = %err, "failed to persist chat context");
        }
        if let Err(err) = self.file.flush().await {
            error!(path = %self.file.file_path().display(), err = %err, "failed to flush chat context");
        }
    }

    pub async fn history(&self) -> Vec<Message> {
        self.history.read().await.clone()
    }
}
