use super::tavily::TavilyError;

#[derive(thiserror::Error, Debug)]
pub enum WebSearchError {
    #[error("websearch tool is not configured: environment variable TAVILY_API_KEY is not set")]
    NotConfigured,
    #[error("failed to perform web search: {0}")]
    SearchError(#[from] TavilyError),
}

impl WebSearchError {
    pub fn not_configured() -> Self {
        Self::NotConfigured
    }

    pub fn search_error(err: TavilyError) -> Self {
        Self::SearchError(err)
    }
}
