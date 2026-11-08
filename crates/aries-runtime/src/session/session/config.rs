use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct SessionConfig {
    pub bare: bool,
}

impl SessionConfig {
    const FILENAME: &'static str = "config.toml";

    pub fn new(bare: bool) -> Self {
        Self { bare }
    }

    pub async fn load(dir: impl AsRef<Path>) -> Self {
        let dir = dir.as_ref();
        let file_path = dir.join(SessionConfig::FILENAME);

        let Ok(content) = tokio::fs::read_to_string(file_path).await else {
            return Self::default();
        };

        toml::from_str::<Self>(&content).unwrap_or_default()
    }

    pub async fn save(&self, dir: impl AsRef<Path>) {
        let dir = dir.as_ref();
        let _ = tokio::fs::create_dir_all(dir).await;

        let Ok(content) = toml::to_string_pretty(self) else { return };
        let file_path = dir.join(Self::FILENAME);
        let _ = tokio::fs::write(file_path, content).await;
    }
}
