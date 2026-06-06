use std::fmt::{Display, Formatter};
use std::path::Path;

use crate::repository::count_files;

pub async fn render(cwd: impl AsRef<Path>) -> Option<String> {
    let files = count_files(cwd).ok()?;

    let size = RepositorySize::new(files);

    let lines = [
        "<repository>".to_owned(),
        format!("  Files: {files}"),
        format!("  Size classification: {size}"),
        "</repository>".to_owned(),
        size.guidance().to_owned(),
    ]
    .join("\n");

    Some(lines)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepositorySize {
    Small,
    Medium,
    Large,
}

impl Display for RepositorySize {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            RepositorySize::Small => write!(f, "small"),
            RepositorySize::Medium => write!(f, "medium"),
            RepositorySize::Large => write!(f, "large"),
        }
    }
}

impl RepositorySize {
    const SMALL_MAX: usize = 300;
    const MEDIUM_MAX: usize = 1000;

    fn new(files: usize) -> Self {
        match files {
            n if n <= Self::SMALL_MAX => Self::Small,
            n if n <= Self::MEDIUM_MAX => Self::Medium,
            _ => Self::Large,
        }
    }

    fn guidance(&self) -> &'static str {
        match self {
            RepositorySize::Small => {
                "This is a small repository. Direct exploration by the main agent is acceptable when helpful."
            },
            RepositorySize::Medium => {
                "This is a medium-sized repository. Prefer targeted search and selective file reading. Use Agent tool for broader exploration when useful."
            },
            RepositorySize::Large => {
                "This is a large repository. Do not broadly inspect the codebase in the main agent. Delegate repository exploration, file discovery, and multi-file reading tasks to Agent tool whenever possible. The main agent should focus on planning, coordination, and final synthesis."
            },
        }
    }
}
