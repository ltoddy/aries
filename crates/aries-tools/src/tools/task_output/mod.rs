mod args;
mod error;
mod output;

use rig::tool::{Tool, ToolContext};
use serde_json::Value;

pub use self::args::TaskOutputArgs;
pub use self::error::TaskOutputError;
pub use self::output::TaskOutputOutput;
use crate::context::ToolContext as AriesToolContext;

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
                    "description": "The background task id returned by Bash background or Monitor"
                },
                "block": {
                    "type": "boolean",
                    "description": "Whether to wait until the task finishes before returning. Defaults to true."
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
