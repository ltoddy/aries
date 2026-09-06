use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex_lite::Regex;
use serde::{Deserialize, Serialize};

use crate::tool::ToolList;

static ARGUMENT_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\$(?:(\d+)|ARGUMENTS)").expect("static regex is valid"));

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CommandDefinition {
    location: PathBuf,
    pub frontmatter: Frontmatter,
    pub body: String,
}

impl CommandDefinition {
    pub fn new(
        location: impl AsRef<Path>,
        frontmatter: Frontmatter,
        body: impl Into<String>,
    ) -> Self {
        let location = location.as_ref();
        let body = body.into();

        Self { location: location.to_owned(), frontmatter, body }
    }

    pub fn location(&self) -> &Path {
        &self.location
    }

    // 没有参数就传递空字符串
    pub fn expand_arguments(&self, arguments: &str) -> String {
        let positional =
            shell_words::split(arguments).unwrap_or_else(|_| vec![arguments.to_owned()]);

        ARGUMENT_PATTERN
            .replace_all(&self.body, |caps: &regex_lite::Captures| {
                if let Some(digits) = caps.get(1) {
                    let index = digits.as_str().parse::<usize>().ok();
                    index
                        .and_then(|i| i.checked_sub(1))
                        .and_then(|i| positional.get(i))
                        .map(String::as_str)
                        .unwrap_or("")
                } else {
                    arguments
                }
            })
            .into_owned()
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Frontmatter {
    pub name: String,
    pub description: String,
    #[serde(rename = "argument-hint")]
    pub argument_hint: Option<String>,
    #[serde(rename = "allowed-tools", default)]
    pub allowed_tools: ToolList,
    // pub model: Option<String>,
    // #[serde(rename = "disable-model-invocation")]
    // pub disable_model_invocation: Option<bool>,
    // #[serde(rename = "user-invocable")]
    // pub user_invocable: Option<bool>,
    // pub context: Option<String>,
    // pub agent: Option<String>,
    // pub hooks: Option<HooksSettings>,
}
