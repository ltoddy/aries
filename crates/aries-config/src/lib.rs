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

enum Provider {
    OpenAICompatible,
    Azure,
}

impl Provider {
    fn label(&self) -> &'static str {
        match self {
            Self::OpenAICompatible => "OpenAI Compatible",
            Self::Azure => "Azure",
        }
    }
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
            Self::Azure(_) => "Azure",
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

fn prompt_required(theme: &ColorfulTheme, prompt: &str) -> anyhow::Result<String> {
    let input: String =
        Input::with_theme(theme).with_prompt(prompt).allow_empty(false).interact_text()?;
    Ok(input.trim().to_owned())
}

pub fn setup() -> anyhow::Result<AriesConfig> {
    println!("Welcome to Aries! Let's set up your AI model configuration.");
    let theme = ColorfulTheme::default();

    let providers = [Provider::OpenAICompatible, Provider::Azure];
    let labels: Vec<_> = providers.iter().map(Provider::label).collect();
    let provider = &providers[Select::with_theme(&theme)
        .with_prompt("provider")
        .items(&labels)
        .default(0)
        .interact()?];

    match provider {
        Provider::OpenAICompatible => {
            let base_url = prompt_required(&theme, "base url")?;
            let api_key = prompt_required(&theme, "api key")?;
            let model = prompt_required(&theme, "model")?;

            Ok(AriesConfig::OpenAICompatible(OpenAICompatibleConfig { api_key, base_url, model }))
        },
        Provider::Azure => {
            let azure_endpoint = prompt_required(&theme, "azure endpoint")?;
            let api_key = prompt_required(&theme, "api key")?;
            let api_version = prompt_required(&theme, "api version")?;
            let model = prompt_required(&theme, "model")?;

            Ok(AriesConfig::Azure(AzureConfig { api_key, azure_endpoint, api_version, model }))
        },
    }
}
