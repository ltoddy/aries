use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct WebSearchArgs {
    pub query: String,
    pub num: Option<i32>,
    pub allowed_domains: Option<Vec<String>>,
    pub blocked_domains: Option<Vec<String>>,
}

impl WebSearchArgs {
    pub fn title(&self) -> String {
        format!("Search the web for {}", self.query)
    }
}

impl WebSearchArgs {
    pub fn render_args(raw: &str) -> Result<(String, Option<String>), serde_json::Error> {
        let args: Self = serde_json::from_str(raw)?;
        let first = args.query;
        Ok((first, None))
    }
}
