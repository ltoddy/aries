use std::io;
use std::path::Path;

use aries_filesystem::jsonl;
use rig_core::completion::Message;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use tracing::{Instrument, Span, error};

#[derive(Debug, Clone)]
pub struct ChatHistory {
    history: Vec<Message>,
    sender: UnboundedSender<Vec<Message>>,
}

impl ChatHistory {
    const FILENAME: &str = "chat-history.jsonl";

    pub async fn new(root_dir: impl AsRef<Path>) -> Self {
        let root_dir = root_dir.as_ref();
        let file_path = root_dir.join(Self::FILENAME);

        let history = Self::load(&file_path).await.unwrap_or_default();

        let (sender, receiver) = unbounded_channel();
        tokio::spawn(refresh_history(receiver, file_path).instrument(Span::current()));

        Self { history, sender }
    }

    pub fn extend(&mut self, messages: impl IntoIterator<Item = Message>) {
        self.history.extend(messages);
        if let Err(err) = self.sender.send(self.history.clone()) {
            error!(err = %err, "failed to send chat history for persistence");
        }
    }

    async fn load(file_path: impl AsRef<Path>) -> io::Result<Vec<Message>> {
        jsonl::read(file_path).await
    }
}

async fn refresh_history(mut rx: UnboundedReceiver<Vec<Message>>, file_path: impl AsRef<Path>) {
    let file_path = file_path.as_ref();

    while let Some(messages) = rx.recv().await {
        if let Some(parent) = file_path.parent() {
            _ = tokio::fs::create_dir_all(parent).await;
        }

        if let Err(err) = jsonl::write(&file_path, &messages).await {
            error!("failed to write chat history to {}: {err}", file_path.display());
        }
    }
}
