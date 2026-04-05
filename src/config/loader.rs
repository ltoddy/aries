use std::path::{Path, PathBuf};

use anyhow::Context;
use directories::ProjectDirs;

use crate::config::{AppConfig, setup};

pub struct AppConfigLoader {
    dir: PathBuf,
    file_path: PathBuf,
}

impl AppConfigLoader {
    const FILE_NAME: &str = "config.json";

    pub async fn new() -> anyhow::Result<Self> {
        let proj_dirs = ProjectDirs::from("", "", env!("CARGO_PKG_NAME"))
            .with_context(|| "Failed to determine project directories")?;
        let config_dir = proj_dirs.config_dir();

        if let Some(parent) = config_dir.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .with_context(|| format!("failed create directory: {}", parent.display()))?;
        }

        let file_path = config_dir.join(Self::FILE_NAME);
        Ok(Self { dir: config_dir.to_path_buf(), file_path })
    }

    pub fn config_dir(&self) -> &Path {
        &self.dir
    }

    pub fn file_path(&self) -> &Path {
        &self.file_path
    }

    pub async fn load_or_setup(&self) -> anyhow::Result<AppConfig> {
        match self.load().await {
            Ok(config) => Ok(config),
            Err(_) => {
                let config = setup()?;
                self.save(&config).await?;
                Ok(config)
            },
        }
    }

    async fn load(&self) -> anyhow::Result<AppConfig> {
        let file_path = self.file_path();

        let config = tokio::fs::read_to_string(file_path)
            .await
            .and_then(|content| serde_json::from_str::<AppConfig>(&content).map_err(Into::into))?;
        Ok(config)
    }

    pub async fn save(&self, config: &AppConfig) -> anyhow::Result<()> {
        let file_path = self.file_path();

        let content = serde_json::to_string_pretty(config)?;
        tokio::fs::write(&file_path, &content).await?;
        Ok(())
    }
}
