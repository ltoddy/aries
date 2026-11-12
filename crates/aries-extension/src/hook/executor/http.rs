use std::str::FromStr;
use std::time::{Duration, TryFromFloatSecsError};

use http::{HeaderName, HeaderValue, StatusCode};
use thiserror::Error;

use crate::hook::definition::HttpHook;
use crate::hook::executor::output::HookOutput;

const DEFAULT_HOOK_TIMEOUT_SECS: f64 = 60.0;

pub async fn execute_http_hook(
    hook: &HttpHook,
    payload: impl Into<String>,
) -> Result<HttpHookOutcome, HttpHookError> {
    let default_headers = hook.headers.as_ref().map_or_else(Default::default, |headers| {
        headers
            .iter()
            .filter_map(|(key, value)| {
                match (HeaderName::from_str(key), HeaderValue::from_str(value)) {
                    (Ok(key), Ok(value)) => Some((key, value)),
                    _ => None,
                }
            })
            .collect()
    });

    let timeout_secs = hook.timeout.unwrap_or(DEFAULT_HOOK_TIMEOUT_SECS);
    let timeout = Duration::try_from_secs_f64(timeout_secs)
        .map_err(|source| HttpHookError::invalid_timeout(timeout_secs, source))?;

    let http_client = reqwest::Client::builder()
        .default_headers(default_headers)
        .timeout(timeout)
        .build()
        .map_err(HttpHookError::build_client)?;

    let payload = payload.into();
    let resp = http_client
        .post(&hook.url)
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(payload)
        .send()
        .await
        .map_err(HttpHookError::send_request)?;

    let status_code = resp.status();
    let body = resp.text().await.map_err(HttpHookError::read_body)?;
    if !status_code.is_success() {
        return Err(HttpHookError::non_success_status(status_code, body));
    }

    if body.trim().is_empty() {
        return Ok(HttpHookOutcome::empty());
    }

    match serde_json::from_str::<HookOutput>(&body) {
        Ok(json) => Ok(HttpHookOutcome::json(json)),
        Err(_) => Ok(HttpHookOutcome::text(body)),
    }
}

#[derive(Debug, Clone)]
pub enum HttpHookOutcome {
    Empty,
    Text(String),
    Json(HookOutput),
}

impl HttpHookOutcome {
    pub fn empty() -> HttpHookOutcome {
        HttpHookOutcome::Empty
    }

    pub fn text(text: impl Into<String>) -> HttpHookOutcome {
        HttpHookOutcome::Text(text.into())
    }

    pub fn json(body: HookOutput) -> HttpHookOutcome {
        HttpHookOutcome::Json(body)
    }
}

#[derive(Debug, Error)]
pub enum HttpHookError {
    #[error("invalid hook timeout {timeout}: {source}")]
    InvalidTimeout { timeout: f64, source: TryFromFloatSecsError },
    #[error("failed to create http client: {0}")]
    BuildClient(#[source] reqwest::Error),
    #[error("failed to send request: {0}")]
    SendRequest(#[source] reqwest::Error),
    #[error("failed to read response body: {0}")]
    ReadBody(#[source] reqwest::Error),
    #[error("hook returned non-success status {status_code}: {body}")]
    NonSuccessStatus { status_code: StatusCode, body: String },
}

impl HttpHookError {
    pub fn invalid_timeout(timeout: f64, source: TryFromFloatSecsError) -> Self {
        Self::InvalidTimeout { timeout, source }
    }

    pub fn build_client(err: reqwest::Error) -> Self {
        Self::BuildClient(err)
    }

    pub fn send_request(err: reqwest::Error) -> Self {
        Self::SendRequest(err)
    }

    pub fn read_body(err: reqwest::Error) -> Self {
        Self::ReadBody(err)
    }

    pub fn non_success_status(status_code: StatusCode, body: impl Into<String>) -> Self {
        Self::NonSuccessStatus { status_code, body: body.into() }
    }
}
