use std::fmt::{Display, Formatter};
use std::path::Path;

pub fn section(cwd: impl AsRef<Path>) -> String {
    let cwd = cwd.as_ref();

    let Ok(_) = git2::Repository::discover(cwd) else {
        return String::from(
            "<repository>This directory is not part of a git repository.</repository>",
        );
    };

    let entries = aries_filesystem::walk::walk_dir(cwd, true, true).unwrap_or_default();
    let files = entries.into_iter().filter(|e| e.is_file()).collect::<Vec<_>>();

    let size = RepositorySize::new(files.len());

    [
        "<repository>",
        "This directory is a git repository.",
        &format!("  Files: {}", files.len()),
        &format!("  Size classification: {size}"),
        size.guidance(),
        "</repository>",
    ]
    .join("\n")
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
