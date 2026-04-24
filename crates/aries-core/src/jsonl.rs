use std::io;
use std::path::Path;

use itertools::Itertools;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Deserializer;

pub async fn write<S: Serialize>(
    path: impl AsRef<Path>,
    elements: impl AsRef<[S]>,
) -> io::Result<()> {
    let content =
        elements.as_ref().iter().filter_map(|ele| serde_json::to_string(ele).ok()).join("\n");

    tokio::fs::write(path, content).await?;
    Ok(())
}

pub async fn read<D: DeserializeOwned>(path: impl AsRef<Path>) -> io::Result<Vec<D>> {
    let content = tokio::fs::read_to_string(path).await?;

    let elements = Deserializer::from_str(&content)
        .into_iter::<D>()
        .filter_map(Result::ok)
        .collect::<Vec<_>>();

    Ok(elements)
}
