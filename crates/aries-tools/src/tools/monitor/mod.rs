mod args;
mod error;
mod output;

use rig::tool::{Tool, ToolContext};
use serde_json::Value;

pub use self::args::MonitorArgs;
pub use self::error::MonitorError;
pub use self::output::MonitorOutput;
use crate::context::{TaskKind, ToolContext as AriesToolContext};

pub const NAME: &str = "Monitor";

pub struct MonitorTool {
    ctx: AriesToolContext,
}

impl MonitorTool {
    pub fn new(ctx: AriesToolContext) -> Self {
        Self { ctx }
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
                    "description": "The shell command to run in the background"
                },
                "description": {
                    "type": "string",
                    "description": "A short description of what this monitor observes"
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
        let cwd = self
            .ctx
            .worktree()
            .map_or_else(|| self.ctx.cwd(), |worktree| worktree.path().to_owned());
        let task =
            self.ctx.task.spawn(TaskKind::Monitor, cwd, args.command, args.description).await?;

        let task_id = task.task_id;
        let message = "Monitor started. Use TaskOutput to read output and TaskStop to stop it.";
        Ok(MonitorOutput::new(task_id, message))
    }
}
