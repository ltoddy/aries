mod args;
mod error;
mod output;
#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use crate::shell::{ShellKind, ShellSpec, detect_shell};
use rig_agent::tool::{Tool, ToolContext};
use serde_json::Value;
use tree_sitter::{Language, Node, Parser};

pub use self::args::BashArgs;
pub use self::error::BashError;
pub use self::output::BashOutput;

pub const NAME: &str = "Bash";

pub struct BashTool {
    cwd: PathBuf,
    language: Language,
    shell: ShellSpec,
}

impl BashTool {
    const DEFAULT_TIMEOUT_MS: u64 = 120_000;
    const MAX_TIMEOUT_MS: u64 = 600_000;

    pub fn new(cwd: impl AsRef<Path>) -> Self {
        let cwd = cwd.as_ref().to_path_buf();
        let language = tree_sitter_bash::LANGUAGE.into();
        let shell = detect_shell();

        Self { cwd, language, shell }
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
        let text = if self.shell.kind == ShellKind::Bash {
            include_str!("description.md")
        } else {
            include_str!("description_windows.md")
        };
        text.to_owned()
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The shell command to execute"
                },
                "timeout": {
                    "type": "number",
                    "description": format!(
                        "Optional timeout in milliseconds (default {}, max {})",
                        Self::DEFAULT_TIMEOUT_MS,
                        Self::MAX_TIMEOUT_MS
                    )
                },
                "description": {
                    "type": "string",
                    "description": "Clear, concise description of what this command does in 5-10 words"
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
        // The tree-sitter-bash rewrite optimization is bash-specific. On
        // PowerShell / cmd we pass the command through verbatim.
        let arg = if self.shell.kind == ShellKind::Bash {
            self.attempt_rewrite_last_command(&args.command).unwrap_or(args.command)
        } else {
            args.command
        };

        let timeout = args
            .timeout
            .filter(|&ms| ms > 0)
            .unwrap_or(Self::DEFAULT_TIMEOUT_MS)
            .min(Self::MAX_TIMEOUT_MS);
        let timeout = Duration::from_millis(timeout);

        let mut command = self.shell.build_command(&arg, &self.cwd);
        command
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        let output = match tokio::time::timeout(timeout, command.output()).await {
            Ok(result) => result.map_err(BashError::Io)?,
            Err(_) => return Err(BashError::Timeout(timeout.as_millis() as u64)),
        };

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
