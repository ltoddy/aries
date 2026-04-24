use std::io;
use std::path::Path;

use aries_core::jsonl;
use rig::completion::Message;
use tokio::sync::mpsc::UnboundedReceiver;

#[inline]
pub async fn load_history<P>(file_path: P) -> io::Result<Vec<Message>>
where
    P: AsRef<Path>,
{
    jsonl::read(file_path).await
}

pub async fn refresh_history<P>(mut rx: UnboundedReceiver<Vec<Message>>, file_path: P)
where
    P: AsRef<Path>,
{
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
