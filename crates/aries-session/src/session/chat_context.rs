use std::io;
use std::path::{Path, PathBuf};

use aries_core::fs::jsonl;
use rig_core::completion::Message;
use tracing::error;

#[derive(Debug, Clone)]
pub struct ChatContext {
    history: Vec<Message>,
    file_path: PathBuf,
}

impl ChatContext {
    const FILENAME: &'static str = "chat-context.jsonl";

    pub async fn new(root_dir: impl AsRef<Path>) -> Self {
        let root_dir = root_dir.as_ref();
        let file_path = root_dir.join(Self::FILENAME);

        let history = Self::load(&file_path).await.unwrap_or_default();

        Self { history, file_path }
    }

    pub async fn extend(&mut self, messages: impl IntoIterator<Item = Message>) {
        self.history.extend(messages);
        if let Err(err) = jsonl::write(&self.file_path, &self.history).await {
            error!("failed to write context history to {}: {err}", self.file_path.display());
        }
    }

    pub async fn overwrite(&mut self, messages: impl IntoIterator<Item = Message>) {
        self.history = messages.into_iter().collect();
        if let Err(err) = jsonl::write(&self.file_path, &self.history).await {
            error!("failed to write context history to {}: {err}", self.file_path.display());
        }
    }

    pub fn history(&self) -> &[Message] {
        &self.history
    }

    pub fn history_mut(&mut self) -> &mut [Message] {
        &mut self.history
    }

    #[inline]
    async fn load(file_path: impl AsRef<Path>) -> io::Result<Vec<Message>> {
        jsonl::read(file_path).await
    }
}
