use std::path::Path;

use fs4::AsyncFileExt;

pub async fn lock(file_path: impl AsRef<Path>) -> std::io::Result<tokio::fs::File> {
    let file = tokio::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(file_path)
        .await?;
    file.lock()?;

    Ok(file)
}

pub async fn try_lock(file_path: impl AsRef<Path>) -> std::io::Result<tokio::fs::File> {
    let file = tokio::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(file_path)
        .await?;
    file.try_lock()?;

    Ok(file)
}
