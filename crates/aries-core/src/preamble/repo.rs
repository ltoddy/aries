use std::fmt::{Display, Formatter};
use std::path::Path;

use crate::simple_cloc;

pub async fn render(cwd: impl AsRef<Path>) -> Option<String> {
    let stats = simple_cloc::calc(cwd).await.ok()?;
    let size = RepoSize::new(&stats);
    let bytes_mb = stats.bytes as f64 / (1024.0 * 1024.0);

    let lines = vec![
        "<repository>".to_string(),
        format!("  Files: {}", stats.files),
        format!("  Code lines: {}", stats.code),
        format!("  Comment lines: {}", stats.comment),
        format!("  Blank lines: {}", stats.blank),
        format!("  Bytes: {} ({bytes_mb:.2} MB)", stats.bytes),
        format!("  Size classification: {size}"),
        "</repository>".to_string(),
        size.guidance(),
    ];

    Some(lines.join("\n"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepoSize {
    Small,
    Medium,
    Large,
}

impl Display for RepoSize {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            RepoSize::Small => write!(f, "small"),
            RepoSize::Medium => write!(f, "medium"),
            RepoSize::Large => write!(f, "large"),
        }
    }
}

impl RepoSize {
    fn new(stats: &simple_cloc::File) -> Self {
        if stats.files > 1000 || stats.code > 120_000 || stats.bytes > 30 * 1024 * 1024 {
            RepoSize::Large
        } else if stats.files > 300 || stats.code > 30_000 || stats.bytes > 10 * 1024 * 1024 {
            RepoSize::Medium
        } else {
            RepoSize::Small
        }
    }

    fn guidance(&self) -> String {
        match self {
            RepoSize::Small => {
                "This is a small repository. Direct exploration by the main agent is acceptable when helpful.".to_owned()
            },
            RepoSize::Medium => {
                "This is a medium-sized repository. Prefer targeted search and selective file reading. Use Agent tool for broader exploration when useful.".to_owned()
            },
            RepoSize::Large => {
                "This is a large repository. Do not broadly inspect the codebase in the main agent. Delegate repository exploration, file discovery, and multi-file reading tasks to Agent tool whenever possible. The main agent should focus on planning, coordination, and final synthesis.".to_owned()
            },
        }
    }
}
