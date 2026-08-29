mod args;
mod error;
mod output;

use rig::tool::{Tool, ToolContext};
use serde_json::Value;

pub use self::args::TaskStopArgs;
pub use self::error::TaskStopError;
pub use self::output::TaskStopOutput;
use crate::context::ToolContext as AriesToolContext;

pub const NAME: &str = "TaskStop";

pub struct TaskStopTool {
    ctx: AriesToolContext,
}

impl TaskStopTool {
    pub fn new(ctx: AriesToolContext) -> Self {
        Self { ctx }
    }
}

impl Tool for TaskStopTool {
    const NAME: &'static str = NAME;
    type Args = TaskStopArgs;
    type Output = TaskStopOutput;
    type Error = TaskStopError;

    fn description(&self) -> String {
        include_str!("description.md").to_owned()
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "task_id": {
                    "type": "string",
                    "description": "The background task id to stop"
                }
            },
            "required": ["task_id"]
        })
    }

    async fn call(
        &self,
        _context: &mut ToolContext,
        args: Self::Args,
    ) -> Result<Self::Output, Self::Error> {
        let snapshot = self.ctx.task.stop(&args.task_id).await?;

        Ok(TaskStopOutput::new(snapshot.task_id, snapshot.kind, snapshot.status, snapshot.command))
    }
}
