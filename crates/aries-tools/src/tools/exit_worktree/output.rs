use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::args::ExitGitWorktreeAction;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExitGitWorktreeOutput {
    pub action: ExitGitWorktreeAction,
    pub original_cwd: PathBuf,
    pub worktree_path: PathBuf,
    pub worktree_branch: String,
    pub message: String,
}

impl ExitGitWorktreeOutput {
    pub fn keep(
        original_cwd: impl AsRef<Path>,
        worktree_path: impl AsRef<Path>,
        worktree_branch: impl Into<String>,
    ) -> Self {
        let original_cwd = original_cwd.as_ref().to_owned();
        let worktree_path = worktree_path.as_ref().to_owned();
        let worktree_branch = worktree_branch.into();
        let message = format!(
            "Exited worktree. Your work is preserved at {} on branch {worktree_branch}. Session is now back in {}.",
            worktree_path.display(),
            original_cwd.display()
        );

        Self {
            action: ExitGitWorktreeAction::Keep,
            original_cwd,
            worktree_path,
            worktree_branch,
            message,
        }
    }

    pub fn remove(
        original_cwd: impl AsRef<Path>,
        worktree_path: impl AsRef<Path>,
        worktree_branch: impl Into<String>,
    ) -> Self {
        let original_cwd = original_cwd.as_ref().to_owned();
        let worktree_path = worktree_path.as_ref().to_owned();
        let worktree_branch = worktree_branch.into();
        let message = format!(
            "Exited and removed worktree at {}. Session is now back in {}.",
            worktree_path.display(),
            original_cwd.display()
        );

        Self {
            action: ExitGitWorktreeAction::Remove,
            original_cwd,
            worktree_path,
            worktree_branch,
            message,
        }
    }

    pub fn render_output(raw: serde_json::Value) -> Result<String, serde_json::Error> {
        let output: Self = serde_json::from_value(raw)?;
        Ok(format!(
            "<action>{:?}</action>\n<original_cwd>{}</original_cwd>\n<worktree_path>{}</worktree_path>\n<worktree_branch>{}</worktree_branch>\n<message>{}</message>",
            output.action,
            output.original_cwd.display(),
            output.worktree_path.display(),
            output.worktree_branch,
            output.message
        ))
    }
}
