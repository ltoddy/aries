use std::io;
use std::path::{Path, PathBuf};

use aries_filesystem::jsonl;
use rig_agent::completion::Message;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use tracing::{Instrument, Span, error};

#[derive(Debug, Clone)]
pub struct ChatContext {
    history: Vec<Message>,
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

        Self { history, sender, file_path: file_path.to_path_buf() }
    }

    pub fn extend(&mut self, messages: impl IntoIterator<Item = Message>) {
        self.history.extend(messages);
        if let Err(err) = self.sender.send(self.history.clone()) {
            error!(err = %err, "failed to send context history for persistence")
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
