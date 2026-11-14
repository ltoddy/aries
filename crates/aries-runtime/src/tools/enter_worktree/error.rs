use std::path::PathBuf;

#[derive(thiserror::Error, Debug)]
pub enum EnterGitWorktreeError {
    #[error("already in a worktree; exit it first")]
    AlreadyInWorktree,
    #[error("invalid worktree name: {0}")]
    InvalidName(String),
    #[error("cannot read HEAD: {0}; check repository state")]
    ReadHead(#[source] git2::Error),
    #[error("cannot create parent directory {path}: {source}")]
    CreateWorktreeParentDir { path: PathBuf, source: std::io::Error },
    #[error("cannot create branch {branch}: {source}; try another name")]
    CreateBranch { branch: String, source: git2::Error },
    #[error("cannot add worktree {}: {source}; try another name or prune stale worktrees", .path.display())]
    AddWorktree { path: PathBuf, source: git2::Error },
    #[error("cannot add worktree {}: {source}; cannot roll back branch {branch}: {rollback}", .path.display())]
    RollbackBranch { path: PathBuf, branch: String, source: git2::Error, rollback: git2::Error },
}

impl EnterGitWorktreeError {
    pub fn already_in_worktree() -> Self {
        Self::AlreadyInWorktree
    }

    pub fn invalid_name(message: impl Into<String>) -> Self {
        Self::InvalidName(message.into())
    }

    pub fn read_head(source: git2::Error) -> Self {
        Self::ReadHead(source)
    }

    pub fn create_worktree_parent_dir(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        let path = path.into();

        Self::CreateWorktreeParentDir { path, source }
    }

    pub fn create_branch(branch: impl Into<String>, source: git2::Error) -> Self {
        Self::CreateBranch { branch: branch.into(), source }
    }

    pub fn add_worktree(path: impl Into<PathBuf>, source: git2::Error) -> Self {
        Self::AddWorktree { path: path.into(), source }
    }

    pub fn rollback_branch(
        path: impl Into<PathBuf>,
        branch: impl Into<String>,
        source: git2::Error,
        rollback: git2::Error,
    ) -> Self {
        Self::RollbackBranch { path: path.into(), branch: branch.into(), source, rollback }
    }
}
