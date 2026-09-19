use std::borrow::Cow;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::tool::ToolList;

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

        shellexpand::env_with_context_no_errors(&self.body, |name| {
            if name == "ARGUMENTS" {
                return Some(Cow::Borrowed(arguments));
            }

            let digits = name.chars().take_while(char::is_ascii_digit).count();
            let (index, suffix) = name.split_at(digits);
            if index.is_empty() {
                return None;
            }

            let value = index
                .parse::<usize>()
                .ok()
                .and_then(|i| i.checked_sub(1))
                .and_then(|i| positional.get(i))
                .map(String::as_str)
                .unwrap_or("");

            Some(Cow::Owned(format!("{value}{suffix}")))
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
