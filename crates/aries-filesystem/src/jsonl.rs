use std::fmt::{Debug, Formatter};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Deserializer;
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;
use tracing::warn;

pub async fn write<'a, S: Serialize + 'a>(
    path: impl AsRef<Path>,
    elements: impl IntoIterator<Item = &'a S>,
) -> io::Result<()> {
    let lines: Vec<String> = elements
        .into_iter()
        .enumerate()
        .map(|(i, ele)| {
            serde_json::to_string(ele).map_err(|e| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("failed to serialize element {i} as JSON: {e}"),
                )
            })
        })
        .collect::<io::Result<Vec<_>>>()?;

    let content = lines.join("\n");
    tokio::fs::write(path, content).await?;
    Ok(())
}

pub async fn read<D: DeserializeOwned>(path: impl AsRef<Path>) -> io::Result<Vec<D>> {
    let path = path.as_ref().to_path_buf();
    let content = tokio::fs::read_to_string(&path).await?;

    let elements: Vec<D> = Deserializer::from_str(&content)
        .into_iter::<D>()
        .enumerate()
        .filter_map(|(i, result)| match result {
            Ok(msg) => Some(msg),
            Err(e) => {
                warn!(path = %path.display(), line = i, error = %e, "dropping line: failed to deserialize");
                None
            },
        })
        .collect::<Vec<_>>();

    Ok(elements)
}

#[derive(Clone)]
pub struct JsonlAppender {
    file_path: PathBuf,
    file: Arc<Mutex<tokio::fs::File>>,
}

impl JsonlAppender {
    pub async fn open(file_path: impl AsRef<Path>) -> std::io::Result<Self> {
        let file_path = file_path.as_ref();
        let file = tokio::fs::OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .append(true)
            .open(file_path)
            .await?;

        Ok(Self { file_path: file_path.to_owned(), file: Arc::new(Mutex::new(file)) })
    }

    pub async fn append<'a, S: Serialize + 'a>(
        &self,
        elements: impl IntoIterator<Item = &'a S>,
    ) -> io::Result<()> {
        let mut guard = self.file.lock().await;
        for (i, element) in elements.into_iter().enumerate() {
            let line = serde_json::to_string(element).map_err(|err| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("failed to serialize element {i} as JSON: {err}"),
                )
            })?;
            guard.write_all(format!("{line}\n").as_bytes()).await?;
        }
        Ok(())
    }

    pub async fn overwrite<'a, S: Serialize + 'a>(
        &self,
        elements: impl IntoIterator<Item = &'a S>,
    ) -> io::Result<()> {
        let mut guard = self.file.lock().await;
        guard.set_len(0).await?;
        for (i, element) in elements.into_iter().enumerate() {
            let line = serde_json::to_string(element).map_err(|err| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("failed to serialize element {i} as JSON: {err}"),
                )
            })?;
            guard.write_all(format!("{line}\n").as_bytes()).await?;
        }
        Ok(())
    }

    pub async fn flush(&self) -> io::Result<()> {
        let mut guard = self.file.lock().await;
        guard.flush().await
    }

    pub fn file_path(&self) -> &Path {
        &self.file_path
    }
}

impl Debug for JsonlAppender {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JsonlAppender").field("file_path", &self.file_path).finish()
    }
}
