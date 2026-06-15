use std::fmt::{Display, Formatter};
use std::path::{Path, PathBuf};

use anyhow::Context;
use prettytable::{Cell, Row, Table, row};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum SettingError {
    #[error("model alias `{0}` already exists")]
    DuplicateModelAlias(String),

    #[error("active model `{0}` not found in configuration")]
    ActiveModelNotFound(String),
}

impl SettingError {
    pub fn duplicate(alias: impl Into<String>) -> Self {
        let alias = alias.into();
        Self::DuplicateModelAlias(alias)
    }

    pub fn not_found(alias: impl Into<String>) -> Self {
        let alias = alias.into();
        Self::ActiveModelNotFound(alias)
    }
}

#[derive(Debug, Clone)]
pub struct SettingLoader {
    file_path: PathBuf,
}

impl SettingLoader {
    pub fn new(root_dir: impl AsRef<Path>) -> Self {
        let root_dir = root_dir.as_ref();
        let file_path = root_dir.join(Setting::FILENAME);

        Self { file_path }
    }

    pub async fn load(&self) -> anyhow::Result<Setting> {
        let file_path = &self.file_path;

        let content = tokio::fs::read_to_string(file_path)
            .await
            .with_context(|| format!("failed to read setting from {}", file_path.display()))?;

        let setting = toml::from_str(&content)
            .with_context(|| format!("failed to parse setting from TOML: {content}"))?;

        Ok(setting)
    }

    pub async fn save(&self, s: &Setting) -> anyhow::Result<()> {
        let file_path = &self.file_path;

        let content = toml::to_string_pretty(s)
            .with_context(|| format!("failed to serialize setting to TOML: {s:?}"))?;

        tokio::fs::write(file_path, &content)
            .await
            .with_context(|| format!("failed to write setting to {}", file_path.display()))?;
        Ok(())
    }

    pub fn file_path(&self) -> impl AsRef<Path> {
        &self.file_path
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct Setting {
    #[serde(default = "default_nickname")]
    pub nickname: String,

    #[serde(default)]
    pub active: String,

    #[serde(default)]
    pub models: Vec<ModelConfig>,
}

#[inline]
fn default_nickname() -> String {
    whoami::realname().unwrap_or_default()
}

impl Default for Setting {
    fn default() -> Self {
        let nickname = whoami::realname().unwrap_or_default();

        Self { nickname, active: String::new(), models: Vec::new() }
    }
}

impl Setting {
    const FILENAME: &str = "setting.toml";

    pub fn new(model: ModelConfig) -> Self {
        let nickname = whoami::realname().unwrap_or_default();

        Self { nickname, active: model.alias().into(), models: vec![model] }
    }

    pub fn add_model(&mut self, model: ModelConfig) {
        if self.models.is_empty() {
            self.active = model.alias().into();
        }
        self.models.push(model);
    }

    #[inline]
    pub fn active_model(&self) -> Result<ModelConfig, SettingError> {
        self.models
            .iter()
            .find(|m| m.alias().into() == self.active)
            .cloned()
            .ok_or_else(|| SettingError::not_found(self.active.clone()))
    }

    pub fn activate(&mut self, alias: impl Into<String>) -> Result<(), SettingError> {
        let alias = alias.into();
        if !self.models.iter().any(|m| m.alias().into() == alias) {
            return Err(SettingError::not_found(alias));
        }

        self.active = alias;
        Ok(())
    }

    #[inline]
    pub fn aliases(&self) -> Vec<String> {
        self.models.iter().map(|m| m.alias().into()).collect::<Vec<_>>()
    }

    pub fn table(&self) -> Table {
        let mut table = Table::new();
        table.add_row(row!["Active", "Alias", "Provider", "Model"]);
        self.models
            .iter()
            .map(|m| {
                Row::new(vec![
                    Cell::new(if m.alias().into() == self.active { "default" } else { "" }),
                    Cell::new(m.alias().into().as_str()),
                    Cell::new(m.provider().as_str()),
                    Cell::new(m.model().into().as_str()),
                ])
            })
            .for_each(|row| {
                table.add_row(row);
            });

        table
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ModelConfig {
    Azure(Azure),
    Deepseek(Deepseek),
    OpenAI(OpenAI),
}

impl ModelConfig {
    pub fn azure(
        alias: impl Into<String>,
        model: impl Into<String>,
        api_key: impl Into<String>,
        azure_endpoint: impl Into<String>,
        api_version: impl Into<String>,
    ) -> Self {
        let alias = alias.into();
        let model = model.into();
        let api_key = api_key.into();
        let azure_endpoint = azure_endpoint.into();
        let api_version = api_version.into();

        Self::Azure(Azure { alias, model, api_key, azure_endpoint, api_version })
    }

    pub fn deepseek(
        alias: impl Into<String>,
        model: impl Into<String>,
        api_key: impl Into<String>,
        base_url: impl Into<String>,
    ) -> Self {
        let alias = alias.into();
        let model = model.into();
        let api_key = api_key.into();
        let base_url = base_url.into();

        Self::Deepseek(Deepseek { alias, model, api_key, base_url })
    }

    pub fn openai(
        alias: impl Into<String>,
        model: impl Into<String>,
        api_key: impl Into<String>,
        base_url: impl Into<String>,
    ) -> Self {
        let alias = alias.into();
        let model = model.into();
        let api_key = api_key.into();
        let base_url = base_url.into();

        Self::OpenAI(OpenAI { alias, model, api_key, base_url })
    }

    pub const fn alias(&self) -> impl Into<String> {
        match self {
            ModelConfig::OpenAI(o) => &o.alias,
            ModelConfig::Azure(a) => &a.alias,
            ModelConfig::Deepseek(d) => &d.alias,
        }
    }

    pub const fn model(&self) -> impl Into<String> {
        match self {
            ModelConfig::OpenAI(o) => &o.model,
            ModelConfig::Azure(a) => &a.model,
            ModelConfig::Deepseek(d) => &d.model,
        }
    }

    pub const fn provider(&self) -> Provider {
        match self {
            ModelConfig::Azure(_) => Provider::Azure,
            ModelConfig::Deepseek(_) => Provider::DeepSeek,
            ModelConfig::OpenAI(_) => Provider::OpenAI,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub enum Provider {
    Azure,
    DeepSeek,
    OpenAI,
}

impl Provider {
    pub const fn as_str(&self) -> &str {
        match self {
            Provider::Azure => "Azure",
            Provider::DeepSeek => "Deepseek",
            Provider::OpenAI => "OpenAI",
        }
    }
}

impl Display for Provider {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Provider::Azure => write!(f, "Azure"),
            Provider::DeepSeek => write!(f, "Deepseek"),
            Provider::OpenAI => write!(f, "OpenAI"),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct OpenAI {
    pub alias: String,
    pub model: String,
    pub api_key: String,
    pub base_url: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct Azure {
    pub alias: String,
    pub model: String,
    pub api_key: String,
    pub azure_endpoint: String,
    pub api_version: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct Deepseek {
    pub alias: String,
    pub model: String,
    pub api_key: String,
    pub base_url: String,
}
