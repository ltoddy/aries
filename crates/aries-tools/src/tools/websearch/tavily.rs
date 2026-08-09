use std::time::Duration;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone)]
pub struct TavilyClient {
    api_key: String,
    http_client: reqwest::Client,
}

impl TavilyClient {
    pub fn new(api_key: impl Into<String>) -> Self {
        let api_key = api_key.into();

        let http_client = reqwest::ClientBuilder::new()
            .user_agent("Aries")
            .timeout(Duration::from_secs(30))
            .build()
            .expect("failed to build Tavily HTTP client");

        Self { api_key, http_client }
    }

    pub async fn search(
        &self,
        request: TavilySearchRequest,
    ) -> Result<TavilySearchResponse, TavilyError> {
        let response = self
            .http_client
            .post("https://api.tavily.com/search")
            .bearer_auth(&self.api_key)
            .json(&request)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(TavilyError::status(status, text));
        }

        response.json().await.map_err(TavilyError::request)
    }
}

#[derive(Debug, Serialize)]
pub struct TavilySearchRequest {
    pub query: String,
    pub max_results: i32,
    pub include_answer: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_domains: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclude_domains: Option<Vec<String>>,
}

impl TavilySearchRequest {
    pub fn new(
        query: impl Into<String>,
        max_results: i32,
        include_answer: bool,
        include_domains: Option<Vec<String>>,
        exclude_domains: Option<Vec<String>>,
    ) -> Self {
        let query = query.into();
        Self { query, max_results, include_answer, include_domains, exclude_domains }
    }
}

#[derive(Debug, Deserialize)]
pub struct TavilySearchResponse {
    pub results: Vec<TavilySearchResult>,
}

#[derive(Debug, Deserialize)]
pub struct TavilySearchResult {
    pub title: Option<String>,
    pub url: Option<String>,
    pub content: Option<String>,
}

#[derive(Debug, Error)]
pub enum TavilyError {
    #[error("Tavily request failed: {0}")]
    Request(#[from] reqwest::Error),

    #[error("Tavily returned {0}: {1}")]
    Status(reqwest::StatusCode, String),
}

impl TavilyError {
    pub fn request(err: reqwest::Error) -> Self {
        Self::Request(err)
    }

    pub fn status(status: reqwest::StatusCode, body: impl Into<String>) -> Self {
        let body = body.into();
        Self::Status(status, body)
    }
}
