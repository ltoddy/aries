use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GlobArgs {
    pub pattern: String,
    pub base_dir: Option<PathBuf>,

    #[serde(default)]
    pub hidden: bool,

    #[serde(default = "default_respect_gitignore")]
    pub respect_gitignore: bool,
}

impl GlobArgs {
    pub fn title(&self) -> String {
        match &self.base_dir {
            Some(base_dir) => {
                format!("Find files matching {} in {}", self.pattern, base_dir.display())
            },
            None => format!("Find files matching {}", self.pattern),
        }
    }
}

impl GlobArgs {
    pub fn render_args(raw: &str) -> Result<(String, Option<String>), serde_json::Error> {
        let args: Self = serde_json::from_str(raw)?;

        let mut first = args.pattern;
        if let Some(base_dir) = args.base_dir {
            first.push_str(&format!(", base_dir = {}", base_dir.display()));
        }

        Ok((first, None))
    }
}

fn default_respect_gitignore() -> bool {
    true
}
