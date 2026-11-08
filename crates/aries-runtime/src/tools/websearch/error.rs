use super::tavily::TavilyError;

#[derive(thiserror::Error, Debug)]
pub enum WebSearchError {
    #[error("websearch tool is not configured: environment variable TAVILY_API_KEY is not set")]
    NotConfigured,
    #[error("failed to perform web search for query {query:?}: {source}")]
    SearchError { query: String, source: TavilyError },
}

impl WebSearchError {
    pub fn not_configured() -> Self {
        Self::NotConfigured
    }

    pub fn search_error(query: impl Into<String>, source: TavilyError) -> Self {
        let query = query.into();

        Self::SearchError { query, source }
    }
}
