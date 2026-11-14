#[derive(thiserror::Error, Debug)]
pub enum ExitGitWorktreeError {
    #[error("no active worktree session")]
    NoActiveWorktree,
    #[error("cannot find worktree {name}: {source}")]
    FindWorktree { name: String, source: git2::Error },
    #[error("cannot remove worktree {}: {source}", .path.display())]
    RemoveWorktree { path: std::path::PathBuf, source: git2::Error },
    #[error(
        "worktree was removed and the session exited, but cannot delete branch {branch}: {source}"
    )]
    DeleteBranch { branch: String, source: git2::Error },
}

impl ExitGitWorktreeError {
    pub fn no_active_worktree() -> Self {
        Self::NoActiveWorktree
    }

    pub fn find_worktree(name: impl Into<String>, source: git2::Error) -> Self {
        Self::FindWorktree { name: name.into(), source }
    }

    pub fn remove_worktree(path: impl Into<std::path::PathBuf>, source: git2::Error) -> Self {
        Self::RemoveWorktree { path: path.into(), source }
    }

    pub fn delete_branch(branch: impl Into<String>, source: git2::Error) -> Self {
        Self::DeleteBranch { branch: branch.into(), source }
    }
}
