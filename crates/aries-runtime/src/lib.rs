pub mod agent;
pub mod compaction;
pub mod hooks;
mod jsonl;
pub mod memory;
pub mod preamble;
pub mod registry;
pub mod session;
pub mod state;
pub mod tools;

use std::time::Duration;

use aries_init::ModelConfig;
use http::{HeaderMap, StatusCode, header};
use reqwest_retry::policies::ExponentialBackoff;
use reqwest_retry::{RetryTransientMiddleware, Retryable, RetryableStrategy};
use rig::Model;
use rig::providers::openai;
use rig::providers::openai::wire::OpenAiWire;
use rig::rig_reqwest::ReqwestMiddlewareClient;

pub fn create_model(config: &ModelConfig) -> Result<Model<OpenAiWire>, session::SessionError> {
    let mut default_headers = HeaderMap::new();
    default_headers.insert("HTTP-Referer", header::HeaderValue::from_static("")); // TODO
    default_headers.insert("X-Title", header::HeaderValue::from_static("Aries"));
    let http_client = reqwest::Client::builder()
        .default_headers(default_headers)
        .connect_timeout(Duration::from_mins(1))
        .build()
        .map_err(session::SessionError::build_client)?;

    let retry = RetryTransientMiddleware::new_with_policy_and_strategy(
        ExponentialBackoff::builder().base(1).build_with_max_retries(5),
        RetryStrategy::new(),
    );
    let http_client = reqwest_middleware::ClientBuilder::new(http_client).with(retry).build();

    let model = config.model().to_owned();

    let config = match config {
        ModelConfig::Azure(c) => openai::OpenAIConfig::with_key(&openai::wire::AZURE, &c.api_key)
            .with_base_url(&c.azure_endpoint)
            .with_api_version(&c.api_version)
            .with_route(openai::Route::Chat),
        ModelConfig::Deepseek(c) => {
            openai::OpenAIConfig::with_key(&openai::wire::DEEPSEEK, &c.api_key)
                .with_base_url(&c.base_url)
                .with_route(openai::Route::Chat)
        },
        ModelConfig::OpenAI(c) => openai::OpenAIConfig::new(&c.api_key)
            .with_base_url(&c.base_url)
            .with_route(openai::Route::Chat),
    };
    Ok(config.connect(ReqwestMiddlewareClient::from(http_client)).completion(model))
}

#[derive(Debug)]
struct RetryStrategy;

impl Default for RetryStrategy {
    fn default() -> Self {
        Self::new()
    }
}

impl RetryStrategy {
    fn new() -> Self {
        Self {}
    }
}

impl RetryableStrategy for RetryStrategy {
    fn handle(
        &self,
        resp: &Result<reqwest::Response, reqwest_middleware::Error>,
    ) -> Option<reqwest_retry::Retryable> {
        let Ok(res) = resp else { return None };

        let status = res.status();
        if status == StatusCode::TOO_MANY_REQUESTS {
            return Some(Retryable::Transient);
        }
        if status.is_server_error() {
            return Some(Retryable::Transient);
        }
        None
    }
}
