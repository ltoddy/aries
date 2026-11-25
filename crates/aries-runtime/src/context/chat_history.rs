use std::io;
use std::path::{Path, PathBuf};

use rig::completion::Message;
use tracing::error;

#[derive(Debug, Clone)]
pub struct ChatHistory {
    file_path: PathBuf,
}

impl ChatHistory {
    const FILENAME: &str = "chat-history.jsonl";

    pub async fn new(root_dir: impl AsRef<Path>) -> io::Result<Self> {
        let root_dir = root_dir.as_ref();
        let file_path = root_dir.join(Self::FILENAME);

        let _ = tokio::fs::create_dir_all(&root_dir).await;
        if let Some(parent) = file_path.parent() {
            let _ = tokio::fs::create_dir_all(parent).await;
        }
        tokio::fs::OpenOptions::new().append(true).create(true).open(&file_path).await?;

        Ok(Self { file_path })
    }

    pub async fn append<'a>(&self, messages: impl IntoIterator<Item = &'a Message>) {
        if let Err(err) = tokio_jsonl::append_all(&self.file_path, messages).await {
            error!(path = %self.file_path.display(), err = %err, "failed to append chat history");
        }
    }

    pub async fn messages(&self) -> io::Result<Vec<Message>> {
        tokio_jsonl::read_all(&self.file_path).await.map_err(io::Error::other)
    }
}
