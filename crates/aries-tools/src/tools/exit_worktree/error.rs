#[derive(thiserror::Error, Debug)]
pub enum ExitGitWorktreeError {
    #[error("no active worktree session")]
    NoActiveWorktree,
    #[error("cannot find worktree: {0}")]
    FindWorktree(String),
    #[error("cannot remove worktree: {0}")]
    RemoveWorktree(String),
    #[error("cannot delete branch: {0}")]
    DeleteBranch(String),
}

impl ExitGitWorktreeError {
    pub fn no_active_worktree() -> Self {
        Self::NoActiveWorktree
    }

    pub fn find_worktree(message: impl Into<String>) -> Self {
        Self::FindWorktree(message.into())
    }

    pub fn remove_worktree(message: impl Into<String>) -> Self {
        Self::RemoveWorktree(message.into())
    }

    pub fn delete_branch(message: impl Into<String>) -> Self {
        Self::DeleteBranch(message.into())
    }
}
