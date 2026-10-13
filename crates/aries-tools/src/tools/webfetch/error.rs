#[derive(thiserror::Error, Debug)]
pub enum WebFetchError {
    #[error(
        "failed to create Firecrawl client for api url {api_url:?}; check FIRECRAWL_API_URL and FIRECRAWL_API_KEY: {source}"
    )]
    CreateClient { api_url: String, source: firecrawl::FirecrawlError },

    #[error("failed to fetch url {url:?} with Firecrawl: {source}")]
    Firecrawl { url: String, source: firecrawl::FirecrawlError },
}

impl WebFetchError {
    pub fn create_client(api_url: impl Into<String>, source: firecrawl::FirecrawlError) -> Self {
        let api_url = api_url.into();

        WebFetchError::CreateClient { api_url, source }
    }

    pub fn firecrawl(url: impl Into<String>, source: firecrawl::FirecrawlError) -> Self {
        let url = url.into();

        Self::Firecrawl { url, source }
    }
}
