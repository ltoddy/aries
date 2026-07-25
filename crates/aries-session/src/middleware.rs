use http::status::StatusCode;
use reqwest_retry::{Retryable, RetryableStrategy};

pub struct RetryStrategy;

impl RetryStrategy {
    pub fn new() -> Self {
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
        None
    }
}
