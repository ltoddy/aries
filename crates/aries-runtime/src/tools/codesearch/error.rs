#[derive(thiserror::Error, Debug)]
pub enum CodeSearchError {
    #[error("failed to send code search request for query {query:?}: {source}")]
    Request { query: String, source: reqwest::Error },

    #[error("code search service returned {status} for query {query:?}: {body}")]
    Status { query: String, status: reqwest::StatusCode, body: String },

    #[error("failed to read code search response body for query {query:?}: {source}")]
    ReadBody { query: String, source: reqwest::Error },
}

impl CodeSearchError {
    pub fn request(query: impl Into<String>, source: reqwest::Error) -> Self {
        let query = query.into();

        Self::Request { query, source }
    }

    pub fn status(
        query: impl Into<String>,
        status: reqwest::StatusCode,
        body: impl Into<String>,
    ) -> Self {
        let query = query.into();
        let body = body.into();

        Self::Status { query, status, body }
    }

    pub fn read_body(query: impl Into<String>, source: reqwest::Error) -> Self {
        let query = query.into();

        Self::ReadBody { query, source }
    }
}
