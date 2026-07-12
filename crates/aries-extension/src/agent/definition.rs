use std::collections::HashSet;
use std::path::{Path, PathBuf};

use itertools::Itertools;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AgentDefinition {
    location: PathBuf,
    pub frontmatter: Frontmatter,
    pub body: String,
}

impl AgentDefinition {
    pub fn new(
        location: impl AsRef<Path>,
        frontmatter: Frontmatter,
        body: impl Into<String>,
    ) -> Self {
        let location = location.as_ref().to_path_buf();
        let body = body.into();
        Self { location, frontmatter, body }
    }

    pub fn location(&self) -> &Path {
        &self.location
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Frontmatter {
    pub name: String,
    pub description: String,
    pub tools: Option<Vec<String>>,
    #[serde(rename = "disallowed-tools")]
    pub disallowed_tools: Option<Vec<String>>,
    pub model: Option<String>,
}

impl Frontmatter {
    pub fn tools_description(&self) -> String {
        let allowed = self.tools.as_deref().filter(|tools| !tools.is_empty());
        let disallowed = self.disallowed_tools.as_deref().filter(|tools| !tools.is_empty());

        match (allowed, disallowed) {
            (Some(allowed), Some(disallowed)) => {
                let effective = allowed
                    .iter()
                    .filter(|tool| !disallowed.contains(tool))
                    .cloned()
                    .collect::<Vec<_>>();
                if effective.is_empty() { "None".to_owned() } else { effective.join(", ") }
            },
            (Some(allowed), None) => allowed.join(", "),
            (None, Some(disallowed)) => format!("All tools except {}", disallowed.join(", ")),
            (None, None) => "All tools".to_owned(),
        }
    }

    pub fn filter_tool_names<'a>(&'a self, all_tools: &[&'a str]) -> Vec<&'a str> {
        let mut tools = match &self.tools {
            Some(tools) if !tools.is_empty() => {
                HashSet::from_iter(tools.iter().map(String::as_str))
            },
            _ => HashSet::from_iter(all_tools.iter().copied()),
        };

        if let Some(disallowed) = &self.disallowed_tools {
            let disallowed = disallowed.iter().map(String::as_str).collect::<HashSet<_>>();
            tools = tools.difference(&disallowed).copied().collect::<HashSet<_>>();
        }

        tools.into_iter().collect_vec()
    }
}
