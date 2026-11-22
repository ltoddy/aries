mod args;
mod error;
mod output;

use aries_init::GlobalContext;
use rig::tool::{Tool, ToolContext};
use serde_json::Value;

pub use self::args::MonitorArgs;
pub use self::error::MonitorError;
pub use self::output::MonitorOutput;
use crate::state::State;
use crate::tools::context::ToolContext as AriesToolContext;
use crate::tools::task::{SpawnTaskError, TaskKind};

pub const NAME: &str = "Monitor";

pub struct MonitorTool {
    ctx: AriesToolContext,
    state: State,
    gctx: GlobalContext,
}

impl MonitorTool {
    pub fn new(ctx: AriesToolContext, state: State, gctx: GlobalContext) -> Self {
        Self { ctx, state, gctx }
    }
}

impl Tool for MonitorTool {
    const NAME: &'static str = NAME;
    type Args = MonitorArgs;
    type Output = MonitorOutput;
    type Error = MonitorError;

    fn description(&self) -> String {
        include_str!("description.md").to_owned()
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "Non-interactive shell command, including any watching loop or repetition needed."
                },
                "description": {
                    "type": "string",
                    "description": "Short user-facing summary of what this monitor observes."
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
        let cwd = self.ctx.cwd();
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "bash".to_owned());
        let command =
            self.state.sandbox_mode().command(&shell, self.gctx.home_dir(), &cwd).await.map_err(
                |source| {
                    SpawnTaskError::new(TaskKind::Monitor, &shell, &cwd, &args.command, source)
                },
            )?;
        let task = self
            .ctx
            .task
            .spawn(TaskKind::Monitor, cwd, args.command, args.description, command)
            .await?;

        let task_id = task.task_id;
        let message = "Monitor started. Use TaskOutput to read output and TaskStop to stop it.";
        Ok(MonitorOutput::new(task_id, message))
    }
}
