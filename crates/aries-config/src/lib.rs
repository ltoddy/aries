use std::path::{Path, PathBuf};

use dialoguer::theme::ColorfulTheme;
use dialoguer::{Input, Select};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum AriesConfig {
    OpenAICompatible(OpenAICompatibleConfig),
    Azure(AzureConfig),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OpenAICompatibleConfig {
    pub api_key: String,
    pub base_url: String,
    pub model: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AzureConfig {
    pub api_key: String,
    pub azure_endpoint: String,
    pub api_version: String,
    pub model: String,
}

impl AriesConfig {
    pub fn provider(&self) -> &'static str {
        match self {
            Self::OpenAICompatible(_) => "OpenAI Compatible",
            Self::Azure(_) => "Azure OpenAI",
        }
    }

    pub fn model(&self) -> &str {
        match self {
            Self::OpenAICompatible(config) => &config.model,
            Self::Azure(config) => &config.model,
        }
    }
}

pub struct AriesConfigLoader {
    file_path: PathBuf,
}

impl AriesConfigLoader {
    const FILE_NAME: &str = "config.toml";

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
        let content = tokio::fs::read_to_string(&self.file_path).await?;
        let config = toml::from_str::<AriesConfig>(&content)?;
        Ok(config)
    }

    pub async fn save(&self, config: &AriesConfig) -> anyhow::Result<()> {
        let content = toml::to_string_pretty(config)?;
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

    let provider = Select::with_theme(&theme)
        .with_prompt("provider")
        .items(["OpenAI Compatible", "Azure OpenAI"])
        .default(0)
        .interact()?;

    match provider {
        0 => {
            let base_url_input: String =
                Input::with_theme(&theme).with_prompt("base url").allow_empty(false).interact_text()?;
            let base_url = base_url_input.trim().to_owned();

            let api_key_input: String =
                Input::with_theme(&theme).with_prompt("api key").allow_empty(false).interact_text()?;
            let api_key = api_key_input.trim().to_owned();

            let model_input: String =
                Input::with_theme(&theme).with_prompt("model").allow_empty(false).interact_text()?;
            let model = model_input.trim().to_owned();

            Ok(AriesConfig::OpenAICompatible(OpenAICompatibleConfig { api_key, base_url, model }))
        },
        _ => {
            let azure_endpoint_input: String =
                Input::with_theme(&theme).with_prompt("azure endpoint").allow_empty(false).interact_text()?;
            let azure_endpoint = azure_endpoint_input.trim().to_owned();

            let api_key_input: String =
                Input::with_theme(&theme).with_prompt("api key").allow_empty(false).interact_text()?;
            let api_key = api_key_input.trim().to_owned();

            let api_version_input: String =
                Input::with_theme(&theme).with_prompt("api version").allow_empty(false).interact_text()?;
            let api_version = api_version_input.trim().to_owned();

            let model_input: String =
                Input::with_theme(&theme).with_prompt("model").allow_empty(false).interact_text()?;
            let model = model_input.trim().to_owned();

            Ok(AriesConfig::Azure(AzureConfig { api_key, azure_endpoint, api_version, model }))
        },
    }
}
