mod args;
mod error;
mod output;

use std::path::{Path, PathBuf};

use aries_init::GlobalContext;
use git2::{Repository, WorktreeAddOptions};
use parking_lot::Mutex;
use regex_lite::Regex;
use rig::tool::{Tool, ToolContext};
use serde_json::Value;

pub use self::args::EnterGitWorktreeArgs;
pub use self::error::EnterGitWorktreeError;
pub use self::output::EnterGitWorktreeOutput;
use crate::context::{ToolContext as AriesToolContext, Worktree};

pub const NAME: &str = "EnterGitWorktree";

pub struct EnterGitWorktreeTool {
    gctx: GlobalContext,
    ctx: AriesToolContext,
    repository: Mutex<Repository>,
    worktree_dir: PathBuf,
    worktree_name_regex: Regex,
}

impl EnterGitWorktreeTool {
    pub fn new(
        gctx: GlobalContext,
        ctx: AriesToolContext,
        repository: Repository,
        worktree_dir: impl AsRef<Path>,
    ) -> Self {
        let worktree_dir = worktree_dir.as_ref();
        let worktree_name_regex = Regex::new(r"^[A-Za-z0-9_-]+(?:/[A-Za-z0-9_-]+)*$")
            .expect("worktree name regex should be valid");

        Self {
            gctx,
            ctx,
            repository: Mutex::new(repository),
            worktree_dir: worktree_dir.to_owned(),
            worktree_name_regex,
        }
    }
}

impl Tool for EnterGitWorktreeTool {
    const NAME: &'static str = NAME;
    type Args = EnterGitWorktreeArgs;
    type Output = EnterGitWorktreeOutput;
    type Error = EnterGitWorktreeError;

    fn description(&self) -> String {
        include_str!("description.md").to_owned()
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Optional name for the worktree. Each segment may contain only letters, digits, underscores, and dashes. A random name is generated if not provided."
                }
            }
        })
    }

    async fn call(
        &self,
        _context: &mut ToolContext,
        args: Self::Args,
    ) -> Result<Self::Output, Self::Error> {
        if self.ctx.worktree().is_some() {
            return Err(EnterGitWorktreeError::already_in_worktree());
        }

        let name = args.name.unwrap_or_else(|| nanoid::nanoid!(10));
        if !self.worktree_name_regex.is_match(&name) {
            return Err(EnterGitWorktreeError::invalid_name(
                "use letters, digits, underscores, dashes, and slashes",
            ));
        }

        let original_cwd = self.ctx.cwd();

        let original_head_commit = {
            let repository = self.repository.lock();
            repository
                .head()
                .and_then(|head| head.peel_to_commit())
                .map(|commit| commit.id().to_string())
                .map_err(|err| EnterGitWorktreeError::read_head(err.message()))?
        };

        let worktree_branch = format!("worktree-{name}");
        let worktree_path = self.worktree_dir.join(&name);

        if let Some(parent) = worktree_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|err| EnterGitWorktreeError::create_worktree_parent_dir(parent, err))?;
        }

        {
            let repository = self.repository.lock();

            let commit = repository
                .head()
                .and_then(|head| head.peel_to_commit())
                .map_err(|err| EnterGitWorktreeError::read_head(err.message()))?;

            let branch = repository
                .branch(&worktree_branch, &commit, false)
                .map_err(|err| EnterGitWorktreeError::create_branch(err.message()))?;

            let mut options = WorktreeAddOptions::new();
            options.reference(Some(branch.get()));
            if let Err(err) = repository.worktree(&worktree_branch, &worktree_path, Some(&options))
            {
                let mut branch = repository
                    .find_branch(&worktree_branch, git2::BranchType::Local)
                    .map_err(|err| EnterGitWorktreeError::rollback_branch(err.message()))?;
                branch
                    .delete()
                    .map_err(|err| EnterGitWorktreeError::rollback_branch(err.message()))?;

                return Err(EnterGitWorktreeError::add_worktree(err.message()));
            }
        }

        let worktree = Worktree::new(
            &original_cwd,
            &worktree_path,
            name,
            &worktree_branch,
            original_head_commit,
        );
        self.ctx.set_worktree(Some(worktree));
        self.ctx.set_cwd(&worktree_path);
        self.gctx.set_current_dir(&worktree_path);

        Ok(EnterGitWorktreeOutput::new(worktree_path, worktree_branch))
    }
}
