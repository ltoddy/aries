use aries_tools::{RenderError, ToolArgsRender};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct AgentArgs {
    pub description: String,
    pub prompt: String,
    pub mode: String,
    pub task_id: Option<String>,
}

impl AgentArgs {
    pub fn title(&self) -> String {
        format!("Launch {} subagent: {}", self.mode, self.description)
    }
}

impl ToolArgsRender for AgentArgs {
    fn render_args(raw: &str) -> Result<(String, Option<String>), RenderError> {
        let args: Self = serde_json::from_str(raw)?;

        let mut first = args.description;
        first.push_str(&format!(", mode = {}", args.mode));
        if let Some(task_id) = &args.task_id {
            first.push_str(&format!(", task_id = {}", task_id));
        }

        Ok((first, Some(args.prompt)))
    }
}
