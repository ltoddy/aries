use std::fmt;
use std::path::PathBuf;

use anyhow::Context;
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Input, Select};
use directories::ProjectDirs;
use rig::providers::{deepseek, openai};
use serde::{Deserialize, Serialize};
use tokio::sync::OnceCell;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    #[serde(rename = "openai")]
    OpenAI,
    #[serde(rename = "deepseek")]
    DeepSeek,
    #[serde(rename = "ollama")]
    Ollama,
    #[serde(rename = "openai_compatible")]
    OpenAICompatible,
}

impl fmt::Display for Provider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Provider::OpenAI => write!(f, "OpenAI"),
            Provider::DeepSeek => write!(f, "DeepSeek"),
            Provider::Ollama => write!(f, "Ollama"),
            Provider::OpenAICompatible => write!(f, "Other (OpenAI Compatible)"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub provider: Provider,
    pub api_key: String,
    pub base_url: Option<String>,
    pub model_name: String,
}

impl AppConfig {
    const FILE_NAME: &str = "config.json";

    pub async fn dir() -> anyhow::Result<PathBuf> {
        static DIR: OnceCell<PathBuf> = OnceCell::const_new();

        let dir = DIR
            .get_or_try_init(async || -> anyhow::Result<PathBuf> {
                let proj_dirs =
                    ProjectDirs::from("", "", "aries").with_context(|| "Failed to determine project directories")?;
                let config_dir = proj_dirs.config_dir();
                if let Some(parent) = config_dir.parent()
                    && !parent.exists()
                {
                    tokio::fs::create_dir_all(parent)
                        .await
                        .with_context(|| format!("failed create directory: {}", parent.display()))?;
                }
                Ok(config_dir.to_path_buf())
            })
            .await?;

        Ok(dir.to_owned())
    }

    pub async fn file_path() -> anyhow::Result<PathBuf> {
        let dir = Self::dir().await?;
        Ok(dir.join(Self::FILE_NAME))
    }

    pub async fn load_or_setup() -> anyhow::Result<Self> {
        match Self::try_load().await {
            Ok(config) => Ok(config),
            Err(_) => {
                let config = Self::setup()?;
                config.save().await?;
                Ok(config)
            },
        }
    }

    async fn try_load() -> anyhow::Result<Self> {
        let file_path = Self::file_path().await?;

        let content = tokio::fs::read_to_string(&file_path).await?;
        let config = serde_json::from_str::<Self>(&content)?;
        Ok(config)
    }

    fn setup() -> anyhow::Result<Self> {
        println!("Welcome to Aries! Let's set up your AI model configuration.");
        let theme = ColorfulTheme::default();

        let providers = [Provider::OpenAI, Provider::DeepSeek, Provider::Ollama, Provider::OpenAICompatible];
        let provider_idx =
            Select::with_theme(&theme).with_prompt("Select a provider").default(0).items(&providers[..]).interact()?;
        let provider = providers[provider_idx].clone();

        let api_key: String = Input::with_theme(&theme)
            .with_prompt("API Key (leave empty if not needed)")
            .allow_empty(true)
            .interact_text()?;

        let default_base_url = match provider {
            Provider::OpenAI => "https://api.openai.com/v1",
            Provider::DeepSeek => "https://api.deepseek.com/v1",
            Provider::Ollama => "http://localhost:11434/v1",
            Provider::OpenAICompatible => "",
        };

        let base_url_input: String =
            Input::with_theme(&theme).with_prompt("Base URL").default(default_base_url.to_string()).interact_text()?;

        let base_url = if base_url_input.trim().is_empty() { None } else { Some(base_url_input.trim().to_string()) };

        let model_name: String = match provider {
            Provider::OpenAI => {
                let models = [
                    openai::GPT_5_2,
                    openai::GPT_5_1,
                    openai::GPT_5,
                    openai::GPT_4_5_PREVIEW,
                    openai::GPT_4O,
                    openai::GPT_4O_MINI,
                    openai::O1,
                    openai::O1_PRO,
                    openai::O1_MINI,
                    openai::O3_MINI,
                    openai::GPT_4_TURBO,
                    openai::GPT_4,
                    "Other (Custom Input)",
                ];
                let model_idx = Select::with_theme(&theme)
                    .with_prompt("Select OpenAI Model")
                    .default(0)
                    .items(&models[..])
                    .interact()?;
                let selected = models[model_idx];
                if selected == "Other (Custom Input)" {
                    Input::with_theme(&theme).with_prompt("Custom Model Name").interact_text()?
                } else {
                    selected.to_string()
                }
            },
            Provider::DeepSeek => {
                let models = [deepseek::DEEPSEEK_CHAT, deepseek::DEEPSEEK_REASONER, "Other (Custom Input)"];
                let model_idx = Select::with_theme(&theme)
                    .with_prompt("Select DeepSeek Model")
                    .default(0)
                    .items(&models[..])
                    .interact()?;
                let selected = models[model_idx];
                if selected == "Other (Custom Input)" {
                    Input::with_theme(&theme).with_prompt("Custom Model Name").interact_text()?
                } else {
                    selected.to_string()
                }
            },
            Provider::Ollama => {
                Input::with_theme(&theme).with_prompt("Model Name").default("llama3".to_string()).interact_text()?
            },
            Provider::OpenAICompatible => Input::with_theme(&theme)
                .with_prompt("Model Name")
                .default(openai::GPT_4O.to_string())
                .interact_text()?,
        };

        Ok(Self { provider, api_key, base_url, model_name })
    }

    async fn save(&self) -> anyhow::Result<()> {
        let file_path = Self::file_path().await?;
        let content = serde_json::to_string_pretty(self)?;
        tokio::fs::write(&file_path, &content).await?;
        Ok(())
    }
}
