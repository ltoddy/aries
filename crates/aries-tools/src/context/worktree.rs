use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Worktree {
    origin_cwd: PathBuf,
    path: PathBuf,
    name: String,
    branch: String,
    head_commit: String,
}

impl Worktree {
    pub fn new(
        origin_cwd: impl AsRef<Path>,
        path: impl AsRef<Path>,
        name: impl Into<String>,
        branch: impl Into<String>,
        head_commit: impl Into<String>,
    ) -> Self {
        let origin_cwd = origin_cwd.as_ref();
        let path = path.as_ref();
        let name = name.into();
        let branch = branch.into();
        let head_commit = head_commit.into();

        Self { origin_cwd: origin_cwd.to_owned(), path: path.to_owned(), name, branch, head_commit }
    }

    pub fn origin_cwd(&self) -> &Path {
        &self.origin_cwd
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn branch(&self) -> &str {
        &self.branch
    }

    pub fn head_commit(&self) -> &str {
        &self.head_commit
    }
}
