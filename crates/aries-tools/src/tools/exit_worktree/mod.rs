mod args;
mod error;
mod output;

use git2::{BranchType, Repository, WorktreePruneOptions};
use parking_lot::Mutex;
use rig::tool::{Tool, ToolContext};
use serde_json::Value;

pub use self::args::{ExitGitWorktreeAction, ExitGitWorktreeArgs};
pub use self::error::ExitGitWorktreeError;
pub use self::output::ExitGitWorktreeOutput;

pub const NAME: &str = "ExitGitWorktree";

pub struct ExitGitWorktreeTool {
    ctx: crate::context::ToolContext,
    repository: Mutex<Repository>,
}

impl ExitGitWorktreeTool {
    pub fn new(ctx: crate::context::ToolContext, repository: Repository) -> Self {
        Self { ctx, repository: Mutex::new(repository) }
    }
}

impl Tool for ExitGitWorktreeTool {
    const NAME: &'static str = NAME;
    type Args = ExitGitWorktreeArgs;
    type Output = ExitGitWorktreeOutput;
    type Error = ExitGitWorktreeError;

    fn description(&self) -> String {
        include_str!("description.md").to_owned()
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["keep", "remove"],
                    "description": "\"keep\" exits and preserves the worktree; \"remove\" exits and deletes the worktree."
                }
            },
            "required": ["action"]
        })
    }

    async fn call(
        &self,
        _context: &mut ToolContext,
        args: Self::Args,
    ) -> Result<Self::Output, Self::Error> {
        let session = self.ctx.worktree().ok_or_else(ExitGitWorktreeError::no_active_worktree)?;

        match args.action {
            ExitGitWorktreeAction::Keep => {
                self.ctx.set_cwd(session.origin_cwd());
                self.ctx.set_worktree(None);

                Ok(ExitGitWorktreeOutput::keep(
                    session.origin_cwd(),
                    session.path(),
                    session.branch(),
                ))
            },
            ExitGitWorktreeAction::Remove => {
                self.ctx.set_cwd(session.origin_cwd());
                self.ctx.set_worktree(None);

                {
                    let repository = self.repository.lock();
                    let worktree = repository
                        .find_worktree(session.name())
                        .map_err(|err| ExitGitWorktreeError::find_worktree(err.message()))?;

                    let mut options = WorktreePruneOptions::new();
                    options.valid(true).working_tree(true);
                    worktree
                        .prune(Some(&mut options))
                        .map_err(|err| ExitGitWorktreeError::remove_worktree(err.message()))?;

                    let mut branch = repository
                        .find_branch(session.branch(), BranchType::Local)
                        .map_err(|err| ExitGitWorktreeError::delete_branch(err.message()))?;
                    branch
                        .delete()
                        .map_err(|err| ExitGitWorktreeError::delete_branch(err.message()))?;
                }

                Ok(ExitGitWorktreeOutput::remove(
                    session.origin_cwd(),
                    session.path(),
                    session.branch(),
                ))
            },
        }
    }
}
