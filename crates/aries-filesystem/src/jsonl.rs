use std::io;
use std::path::Path;

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Deserializer;
use tracing::warn;

pub async fn write<S: Serialize>(
    path: impl AsRef<Path>,
    elements: impl AsRef<[S]>,
) -> io::Result<()> {
    let lines: Vec<String> = elements
        .as_ref()
        .iter()
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
                warn!("dropping line {i} from {}: failed to deserialize — {e}", path.display());
                None
            },
        })
        .collect::<Vec<_>>();

    Ok(elements)
}
