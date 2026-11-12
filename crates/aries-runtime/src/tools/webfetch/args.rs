use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WebFetchArgs {
    pub url: String,
}

impl WebFetchArgs {
    pub fn title(&self) -> String {
        format!("Fetch URL {}", self.url)
    }
}

impl WebFetchArgs {
    pub fn render_args(
        raw: impl AsRef<str>,
    ) -> Result<(String, Option<String>), serde_json::Error> {
        let args: Self = serde_json::from_str(raw.as_ref())?;
        let first = args.url;
        Ok((first, None))
    }
}
