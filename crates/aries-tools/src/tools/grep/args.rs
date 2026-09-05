use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GrepArgs {
    pub pattern: String,
    pub include: Option<String>,

    #[serde(default)]
    pub output_mode: OutputMode,

    #[serde(default)]
    pub case_insensitive: bool,

    #[serde(default = "default_show_line_numbers")]
    pub show_line_numbers: bool,

    pub context_before: Option<usize>,
    pub context_after: Option<usize>,
    pub context: Option<usize>,

    #[serde(default)]
    pub hidden: bool,

    #[serde(default = "default_respect_ignore")]
    pub respect_ignore: bool,

    #[serde(default = "default_limit")]
    pub limit: usize,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputMode {
    Content,
    #[default]
    FilesWithMatches,
    Count,
}

fn default_show_line_numbers() -> bool {
    true
}

fn default_respect_ignore() -> bool {
    true
}

fn default_limit() -> usize {
    250
}
