use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct GrepArgs {
    pub pattern: String,
    pub include: Option<String>,
}

impl GrepArgs {
    pub fn title(&self) -> String {
        match &self.include {
            Some(include) => format!("Search for {} in {}", self.pattern, include),
            None => format!("Search for {} in files", self.pattern),
        }
    }
}

impl GrepArgs {
    pub fn render_args(raw: &str) -> Result<(String, Option<String>), serde_json::Error> {
        let args: Self = serde_json::from_str(raw)?;

        let mut first = args.pattern;
        if let Some(include) = args.include {
            first.push_str(&format!(", include = {include}"));
        }

        Ok((first, None))
    }
}
