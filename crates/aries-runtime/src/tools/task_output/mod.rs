mod args;
mod error;
mod output;

use rig::tool::{Tool, ToolContext};
use serde_json::Value;

pub use self::args::TaskOutputArgs;
pub use self::error::TaskOutputError;
pub use self::output::TaskOutputOutput;
use crate::tools::context::ToolContext as AriesToolContext;

pub const NAME: &str = "TaskOutput";

pub struct TaskOutputTool {
    ctx: AriesToolContext,
}

impl TaskOutputTool {
    pub fn new(ctx: AriesToolContext) -> Self {
        Self { ctx }
    }
}

impl Tool for TaskOutputTool {
    const NAME: &'static str = NAME;
    type Args = TaskOutputArgs;
    type Output = TaskOutputOutput;
    type Error = TaskOutputError;

    fn description(&self) -> String {
        include_str!("description.md").to_owned()
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "task_id": {
                    "type": "string",
                    "description": "Exact task_id returned by Bash with background=true or Monitor in this session, or by their task notification. Not a process PID or an Agent task_id."
                },
                "block": {
                    "type": "boolean",
                    "description": "Wait for the task to finish; defaults to true. false returns the current status and retained-output snapshot immediately."
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
        let snapshot = if args.block {
            self.ctx
                .task
                .wait_until_finished(&args.task_id)
                .await
                .ok_or_else(|| TaskOutputError::not_found(args.task_id))?
        } else {
            self.ctx
                .task
                .get(&args.task_id)
                .ok_or_else(|| TaskOutputError::not_found(args.task_id))?
        };

        Ok(snapshot.into())
    }
}
