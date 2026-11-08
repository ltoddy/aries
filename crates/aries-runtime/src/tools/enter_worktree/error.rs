use std::path::PathBuf;

#[derive(thiserror::Error, Debug)]
pub enum EnterGitWorktreeError {
    #[error("already in a worktree; exit it first")]
    AlreadyInWorktree,
    #[error("not in a non-bare git repository")]
    NotGitRepository,
    #[error("invalid worktree name: {0}")]
    InvalidName(String),
    #[error("cannot read HEAD: {0}; check repository state")]
    ReadHead(String),
    #[error("cannot create parent directory {path}: {source}")]
    CreateWorktreeParentDir { path: PathBuf, source: std::io::Error },
    #[error("cannot create branch: {0}; try another name")]
    CreateBranch(String),
    #[error("cannot add worktree: {0}; try another name or prune stale worktrees")]
    AddWorktree(String),
    #[error("cannot roll back branch after worktree creation failed: {0}")]
    RollbackBranch(String),
}

impl EnterGitWorktreeError {
    pub fn already_in_worktree() -> Self {
        Self::AlreadyInWorktree
    }

    pub fn not_git_repository() -> Self {
        Self::NotGitRepository
    }

    pub fn invalid_name(message: impl Into<String>) -> Self {
        Self::InvalidName(message.into())
    }

    pub fn read_head(message: impl Into<String>) -> Self {
        Self::ReadHead(message.into())
    }

    pub fn create_worktree_parent_dir(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        let path = path.into();

        Self::CreateWorktreeParentDir { path, source }
    }

    pub fn create_branch(message: impl Into<String>) -> Self {
        Self::CreateBranch(message.into())
    }

    pub fn add_worktree(message: impl Into<String>) -> Self {
        Self::AddWorktree(message.into())
    }

    pub fn rollback_branch(message: impl Into<String>) -> Self {
        Self::RollbackBranch(message.into())
    }
}
