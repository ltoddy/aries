pub mod document;
pub mod jsonl;
pub mod lock;
pub mod walk;

use std::path::{Path, PathBuf};

use url::Url;

pub async fn path_to_uri(path: impl AsRef<Path>) -> String {
    let path = path.as_ref();
    let path =
        tokio::fs::canonicalize(path).await.unwrap_or_else(|_| PathBuf::from("/").join(path));

    Url::from_file_path(&path)
        .map(|u| u.to_string())
        .unwrap_or_else(|_| format!("file://{}", path.display()))
}

pub fn path_to_slug(path: impl AsRef<Path>) -> String {
    let path = path.as_ref();
    let path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_owned());

    path.to_string_lossy()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
}
