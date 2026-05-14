use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProjectEntry {
    pub id: String,
    pub name: String,
    pub path: String,
    pub branch: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SessionSummary {
    pub id: String,
    pub session_id: String,
    pub title: Option<String>,
    pub project_dir: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionBootstrap {
    pub app_name: &'static str,
    pub provider: String,
    pub model: String,
    pub session_id: String,
    pub session_dir_name: String,
    pub messages: Vec<ChatMessage>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatRequest {
    pub prompt: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ChatBlock {
    #[serde(rename = "type")]
    pub kind: String,
    pub content: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocks: Option<Vec<ChatBlock>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatResponse {
    pub session_id: String,
    pub message: ChatMessage,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ChatStreamPayload {
    pub seq: u64,
    pub session_id: String,
    pub kind: String,
    pub delta: String,
}

// Model config form data kept locally in aries-ui since it is UI-specific.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ConfigFormData {
    pub provider: String,
    pub api_key: String,
    pub model: String,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub azure_endpoint: Option<String>,
    #[serde(default)]
    pub api_version: Option<String>,
}

impl ConfigFormData {
    pub fn from_config(config: &aries_config::AriesConfig) -> Self {
        match config {
            aries_config::AriesConfig::OpenAICompatible(c) => Self {
                provider: "openai-compatible".to_string(),
                api_key: c.api_key.clone(),
                model: c.model.clone(),
                base_url: Some(c.base_url.clone()),
                azure_endpoint: None,
                api_version: None,
            },
            aries_config::AriesConfig::Azure(c) => Self {
                provider: "azure".to_string(),
                api_key: c.api_key.clone(),
                model: c.model.clone(),
                base_url: None,
                azure_endpoint: Some(c.azure_endpoint.clone()),
                api_version: Some(c.api_version.clone()),
            },
            aries_config::AriesConfig::DeepSeek(c) => Self {
                provider: "deepseek-v4".to_string(),
                api_key: c.api_key.clone(),
                model: c.model.clone(),
                base_url: None,
                azure_endpoint: None,
                api_version: None,
            },
        }
    }

    pub fn into_config(self) -> anyhow::Result<aries_config::AriesConfig> {
        match self.provider.as_str() {
            "openai-compatible" | "deepseek-v4" => {
                let base_url = match self.provider.as_str() {
                    "deepseek-v4" => self
                        .base_url
                        .filter(|s| !s.trim().is_empty())
                        .unwrap_or_else(|| "https://api.deepseek.com".to_string()),
                    _ => self.base_url.ok_or_else(|| anyhow::anyhow!("base_url is required"))?,
                };
                Ok(aries_config::AriesConfig::OpenAICompatible(
                    aries_config::OpenAICompatibleConfig {
                        api_key: self.api_key,
                        base_url,
                        model: self.model,
                    },
                ))
            },
            "azure" => Ok(aries_config::AriesConfig::Azure(aries_config::AzureConfig {
                api_key: self.api_key,
                azure_endpoint: self
                    .azure_endpoint
                    .ok_or_else(|| anyhow::anyhow!("azure_endpoint is required"))?,
                api_version: self
                    .api_version
                    .ok_or_else(|| anyhow::anyhow!("api_version is required"))?,
                model: self.model,
            })),
            other => Err(anyhow::anyhow!("unknown provider: {other}")),
        }
    }
}
