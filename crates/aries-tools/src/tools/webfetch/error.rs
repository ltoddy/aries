#[derive(thiserror::Error, Debug)]
pub enum WebFetchError {
    #[error("missing `FIRECRAWL_API_URL` or `FIRECRAWL_API_KEY` environment variable: {0}")]
    MissingApiKey(firecrawl::FirecrawlError),

    #[error("firecrawl api error: {0}")]
    Firecrawl(firecrawl::FirecrawlError),
}

impl WebFetchError {
    pub fn missing_api_key(err: firecrawl::FirecrawlError) -> Self {
        WebFetchError::MissingApiKey(err)
    }

    pub fn firecrawl(err: firecrawl::FirecrawlError) -> Self {
        Self::Firecrawl(err)
    }
}
