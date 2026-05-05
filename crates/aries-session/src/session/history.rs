use std::io;
use std::path::Path;

use aries_core::fs::jsonl;
use rig::completion::Message;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use tracing::{error, warn};

#[derive(Debug)]
pub struct ChatHistory {
    history: Vec<Message>,
    sender: UnboundedSender<Vec<Message>>,
}

impl ChatHistory {
    pub async fn new(file_path: impl AsRef<Path>) -> Self {
        let file_path = file_path.as_ref();
        if let Some(parent) = file_path.parent() {
            if let Err(err) = tokio::fs::create_dir_all(parent).await {
                error!("failed to create chat history parent directory: {err}");
            }
        }

        let mut history = vec![];
        match Self::load(file_path).await {
            Ok(prior) => history = prior,
            Err(err) => warn!("failed to load chat history: {err}"),
        }

        let (sender, receiver) = unbounded_channel();

        tokio::spawn(refresh_history(receiver, file_path.to_path_buf()));

        Self { history, sender }
    }

    pub fn reset(&mut self, history: &[Message]) {
        self.history = history.to_vec();

        if let Err(err) = self.sender.send(history.to_vec()) {
            error!("failed to send chat history for persistence: {err}")
        };
    }

    pub fn history(&self) -> &[Message] {
        &self.history
    }

    pub fn history_mut(&mut self) -> &mut Vec<Message> {
        &mut self.history
    }

    pub fn push(&mut self, message: Message) {
        self.history.push(message);
    }

    pub fn extend(&mut self, messages: impl IntoIterator<Item = Message>) {
        self.history.extend(messages);
    }

    pub fn persist(&self) {
        if let Err(err) = self.sender.send(self.history.clone()) {
            error!("failed to send chat history for persistence: {err}");
        }
    }

    pub fn clear(&mut self) {
        self.reset(&[]);
    }

    async fn load(file_path: impl AsRef<Path>) -> io::Result<Vec<Message>> {
        jsonl::read(file_path).await
    }
}

async fn refresh_history(mut rx: UnboundedReceiver<Vec<Message>>, file_path: impl AsRef<Path>) {
    let file_path = file_path.as_ref();
    while let Some(messages) = rx.recv().await {
        if let Err(err) = jsonl::write(file_path, messages).await {
            error!("failed to write chat history to {}: {err}", file_path.display());
        }
    }
}
