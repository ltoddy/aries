use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EnterGitWorktreeOutput {
    pub worktree_path: PathBuf,
    pub worktree_branch: String,
    pub message: String,
}

impl EnterGitWorktreeOutput {
    pub fn new(worktree_path: impl AsRef<Path>, worktree_branch: impl Into<String>) -> Self {
        let worktree_path = worktree_path.as_ref();
        let worktree_branch = worktree_branch.into();
        let message = format!(
            "Created worktree at {} on branch {worktree_branch}. The session is now working in the worktree. Use ExitGitWorktree tool to leave it.",
            worktree_path.display(),
        );

        Self { worktree_path: worktree_path.to_owned(), worktree_branch, message }
    }

    pub fn render_output(raw: serde_json::Value) -> Result<String, serde_json::Error> {
        let output: Self = serde_json::from_value(raw)?;
        Ok(format!(
            "<worktree_path>{}</worktree_path>\n<worktree_branch>{}</worktree_branch>\n<message>{}</message>",
            output.worktree_path.display(),
            output.worktree_branch,
            output.message
        ))
    }
}
