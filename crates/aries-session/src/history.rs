use std::io;
use std::path::Path;

use aries_core::jsonl;
use rig::completion::Message;
use tokio::sync::mpsc::UnboundedReceiver;

#[inline]
pub async fn load_history(file_path: impl AsRef<Path>) -> io::Result<Vec<Message>> {
    jsonl::read(file_path).await
}

pub async fn refresh_history(mut rx: UnboundedReceiver<Vec<Message>>, file_path: impl AsRef<Path>) {
    let file_path = file_path.as_ref();
    while let Some(messages) = rx.recv().await {
        if let Some(parent) = file_path.parent()
            && !parent.exists()
        {
            let _ = tokio::fs::create_dir_all(parent).await;
        }

        let _ = jsonl::write(file_path, messages).await;
    }
}
