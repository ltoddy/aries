use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub description: String,
}

impl SearchResult {
    pub fn new(
        title: impl Into<String>,
        url: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        let title = title.into();
        let url = url.into();
        let description = description.into();
        Self { title, url, description }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct WebSearchOutput {
    pub query: String,
    pub results: Vec<SearchResult>,
    pub duration_seconds: f64,
}

impl WebSearchOutput {
    pub fn new(
        query: impl Into<String>,
        results: Vec<SearchResult>,
        duration_seconds: f64,
    ) -> Self {
        let query = query.into();
        Self { query, results, duration_seconds }
    }

    pub fn render_output(raw: serde_json::Value) -> Result<String, serde_json::Error> {
        let output: Self = serde_json::from_value(raw)?;

        if output.results.is_empty() {
            return Ok("No search results found. Please try a different query.".to_owned());
        }

        let mut text = format!("Query: {}\n", output.query);
        for (i, result) in output.results.iter().enumerate() {
            text.push_str(&format!(
                "{}. [{}]({})\n   {}\n",
                i + 1,
                result.title,
                result.url,
                result.description
            ));
        }
        Ok(text)
    }
}
