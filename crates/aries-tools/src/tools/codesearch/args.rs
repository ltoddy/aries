use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CodeSearchArgs {
    pub query: String,
    pub tokens_num: Option<i32>,
}

impl CodeSearchArgs {
    pub fn title(&self) -> String {
        format!("Search code mod for {}", self.query)
    }
}

impl CodeSearchArgs {
    pub fn render_args(raw: &str) -> Result<(String, Option<String>), serde_json::Error> {
        let args: Self = serde_json::from_str(raw)?;

        let mut first = args.query;
        if let Some(token) = args.tokens_num {
            first.push_str(&format!(" token = {token}"));
        }

        Ok((first, None))
    }
}
