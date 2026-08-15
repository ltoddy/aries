use std::io;
use std::path::Path;

use aries_filesystem::jsonl::JsonlAppender;
use rig_agent::completion::Message;
use tracing::error;

#[derive(Debug, Clone)]
pub struct ChatHistory {
    file: JsonlAppender,
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
        let file = JsonlAppender::open(&file_path).await?;

        Ok(Self { file })
    }

    pub async fn append(&self, messages: impl IntoIterator<Item = &Message>) {
        if let Err(err) = self.file.append(messages).await {
            error!(path = %self.file.file_path().display(), err = %err, "failed to append chat history");
            return
        }
        let _ = self.file.flush().await;
    }
}
