use std::path::{Path, PathBuf};

use dialoguer::Input;
use dialoguer::theme::ColorfulTheme;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AriesConfig {
    pub api_key: String,
    pub base_url: String,
    pub model: String,
}

pub struct AriesConfigLoader {
    file_path: PathBuf,
}

impl AriesConfigLoader {
    const FILE_NAME: &str = "config.json";

    pub fn new(config_dir: &Path) -> Self {
        let file_path = config_dir.join(Self::FILE_NAME);

        Self { file_path }
    }

    pub async fn load_or_setup(&self) -> anyhow::Result<AriesConfig> {
        match self.load().await {
            Ok(config) => Ok(config),
            Err(_) => {
                let config = setup()?;
                self.save(&config).await?;
                Ok(config)
            },
        }
    }

    async fn load(&self) -> anyhow::Result<AriesConfig> {
        let config = tokio::fs::read_to_string(&self.file_path)
            .await
            .and_then(|content| serde_json::from_str::<AriesConfig>(&content).map_err(Into::into))?;
        Ok(config)
    }

    pub async fn save(&self, config: &AriesConfig) -> anyhow::Result<()> {
        let content = serde_json::to_string_pretty(config)?;
        tokio::fs::write(&self.file_path, &content).await?;
        Ok(())
    }

    pub fn file_path(&self) -> &Path {
        &self.file_path
    }
}

pub fn setup() -> anyhow::Result<AriesConfig> {
    println!("Welcome to Aries! Let's set up your AI model configuration.");
    let theme = ColorfulTheme::default();

    let base_url_input: String =
        Input::with_theme(&theme).with_prompt("base url").allow_empty(false).interact_text()?;
    let base_url = base_url_input.trim().to_owned();

    let api_key_input: String = Input::with_theme(&theme).with_prompt("api key").allow_empty(false).interact_text()?;
    let api_key = api_key_input.trim().to_owned();

    let model_input: String = Input::with_theme(&theme).with_prompt("model").allow_empty(false).interact_text()?;
    let model = model_input.trim().to_owned();

    Ok(AriesConfig { api_key, base_url, model })
}
