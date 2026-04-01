use std::path::PathBuf;

use anyhow::Context;
use directories::ProjectDirs;

use crate::config::{AppConfig, setup};

pub struct AppConfigLoader {
    dir: PathBuf,
}

impl AppConfigLoader {
    const FILE_NAME: &str = "config.json";

    pub async fn new() -> anyhow::Result<Self> {
        let proj_dirs =
            ProjectDirs::from("", "", "aries").with_context(|| "Failed to determine project directories")?;
        let config_dir = proj_dirs.config_dir();

        if let Some(parent) = config_dir.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .with_context(|| format!("failed create directory: {}", parent.display()))?;
        }

        Ok(Self { dir: config_dir.to_path_buf() })
    }

    pub fn config_dir(&self) -> PathBuf {
        self.dir.clone()
    }

    pub fn file_path(&self) -> PathBuf {
        self.dir.join(Self::FILE_NAME)
    }

    pub async fn load_or_setup(&self) -> anyhow::Result<AppConfig> {
        match self.try_load().await {
            Ok(config) => Ok(config),
            Err(_) => {
                let config = setup()?;
                self.save(&config).await?;
                Ok(config)
            },
        }
    }

    async fn try_load(&self) -> anyhow::Result<AppConfig> {
        let file_path = self.file_path();

        let config = tokio::fs::read_to_string(file_path)
            .await
            .and_then(|content| serde_json::from_str::<AppConfig>(&content).map_err(Into::into))?;
        Ok(config)
    }

    async fn save(&self, config: &AppConfig) -> anyhow::Result<()> {
        let file_path = self.file_path();

        let content = serde_json::to_string_pretty(config)?;
        tokio::fs::write(&file_path, &content).await?;
        Ok(())
    }
}
