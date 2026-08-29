mod args;
mod error;
mod output;
#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};
use std::process::Stdio;

use rig::tool::{Tool, ToolContext};
use serde_json::Value;
use tokio::process::Command;
use tree_sitter::{Language, Node, Parser};

pub use self::args::BashArgs;
pub use self::error::BashError;
pub use self::output::BashOutput;
use crate::context::{self, TaskKind};

pub const NAME: &str = "Bash";

pub struct BashTool {
    cwd: PathBuf,
    ctx: context::ToolContext,
    language: Language,
}

impl BashTool {
    pub fn new(cwd: impl AsRef<Path>, ctx: context::ToolContext) -> Self {
        let cwd = cwd.as_ref();
        let language = tree_sitter_bash::LANGUAGE.into();

        Self { cwd: cwd.to_owned(), ctx, language }
    }

    fn attempt_rewrite_last_command(&self, cmd: &str) -> Option<String> {
        let mut parser = Parser::new();
        parser.set_language(&self.language).ok()?;

        let tree = parser.parse(cmd, None)?;
        let last = find_last_node(tree.root_node())?;

        let pos = last.start_byte();

        Some(format!("{}aries exec {}", &cmd[..pos], &cmd[pos..]))
    }
}

impl Tool for BashTool {
    const NAME: &'static str = NAME;
    type Args = BashArgs;
    type Output = BashOutput;
    type Error = BashError;

    fn description(&self) -> String {
        include_str!("description.md").to_owned()
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The shell command to execute"
                },
                "description": {
                    "type": "string",
                    "description": "Clear, concise description of what this command does in 5-10 words"
                },
                "background": {
                    "type": "boolean",
                    "description": "Set to true to run this command in the background. Use TaskOutput to read the output later."
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
        let arg = self.attempt_rewrite_last_command(&args.command).unwrap_or(args.command);

        if args.background {
            let task =
                self.ctx.task.spawn(TaskKind::Bash, &self.cwd, arg, args.description).await?;
            return Ok(BashOutput::background(task.task_id));
        }

        let shell = std::env::var("SHELL").unwrap_or_else(|_| "bash".to_owned());
        let mut command = Command::new(shell);
        command
            .arg("-c")
            .arg(arg)
            .current_dir(&self.cwd)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        let output = command.output().await.map_err(BashError::io)?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let exit_code = output.status.code().unwrap_or(-1);

        Ok(BashOutput::new(stdout, stderr, exit_code))
    }
}

fn find_last_node(node: Node) -> Option<Node> {
    let mut last = None::<Node>;

    for child in node.children(&mut node.walk()) {
        if child.kind() == "command" {
            last = Some(child);
        }

        if let Some(node) = find_last_node(child) {
            last = Some(node);
        }
    }

    last
}
