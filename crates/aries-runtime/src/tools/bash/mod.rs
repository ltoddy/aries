mod args;
mod error;
mod output;
#[cfg(test)]
mod tests;

#[cfg(target_os = "macos")]
use std::ffi::OsString;
use std::process::Stdio;

pub use self::args::BashArgs;
pub use self::error::BashError;
pub use self::output::BashOutput;
use crate::state::State;
use crate::tools::context::ToolContext as AriesToolContext;
use crate::tools::task::TaskKind;
use aries_init::GlobalContext;
use rig::tool::{Tool, ToolContext};
use serde_json::Value;
use tokio::process::Command;

pub const NAME: &str = "Bash";

pub struct BashTool {
    ctx: AriesToolContext,
    state: State,
    gctx: GlobalContext,
}

impl BashTool {
    pub fn new(ctx: AriesToolContext, state: State, gctx: GlobalContext) -> Self {
        Self { ctx, state, gctx }
    }
}

impl Tool for BashTool {
    const NAME: &'static str = NAME;
    type Args = BashArgs;
    type Output = BashOutput;
    type Error = BashError;

    fn description(&self) -> String {
        let Ok(_) = git2::Repository::discover(self.ctx.cwd()) else {
            return include_str!("description.md").to_owned();
        };

        [include_str!("description.md"), include_str!("description-git.md")].join("\n")
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "Non-interactive shell command. Quote paths containing spaces; join dependent commands with &&."
                },
                "description": {
                    "type": "string",
                    "description": "Short user-facing summary of the command purpose, e.g. Run unit tests."
                },
                "background": {
                    "type": "boolean",
                    "description": "Run in the background instead of waiting for completion. Defaults to false."
                }
            },
            "required": ["command"]
        })
    }

    async fn call(
        &self,
        _context: &mut ToolContext,
        args: Self::Args,
    ) -> Result<Self::Output, Self::Error> {
        let arg = args.command;
        let cwd = self
            .ctx
            .worktree()
            .map_or_else(|| self.ctx.cwd(), |worktree| worktree.path().to_owned());

        if args.background {
            let task = self.ctx.task.spawn(TaskKind::Shell, cwd, arg, args.description).await?;
            return Ok(BashOutput::background(task.task_id));
        }

        let shell = std::env::var("SHELL").unwrap_or_else(|_| "bash".to_owned());
        #[cfg(target_os = "macos")]
        let mut command = {
            let temp_dir = std::env::temp_dir();
            let mut command = Command::new("/usr/bin/sandbox-exec");
            command.arg("-p").arg(self.state.sandbox_mode.seatbelt_profile());

            let params = [
                ("HOME", self.gctx.home_dir()),
                ("WORKSPACE", &cwd),
                ("SANDBOX_TMP", temp_dir.as_path()),
            ];
            for (name, path) in params {
                let mut parameter = OsString::from(name);
                parameter.push("=");
                parameter.push(path);
                command.arg("-D").arg(parameter);
            }
            command.env("TMPDIR", &temp_dir).arg(&shell);
            command
        };

        #[cfg(not(target_os = "macos"))]
        let mut command = Command::new(&shell);
        command
            .arg("-c")
            .arg(&arg)
            .current_dir(&cwd)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        let output = command.output().await.map_err(|err| BashError::io(&shell, cwd, arg, err))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let exit_code = output.status.code().unwrap_or(-1);

        Ok(BashOutput::new(stdout, stderr, exit_code))
    }
}
