use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TaskStopArgs {
    pub task_id: String,
}

impl TaskStopArgs {
    pub fn title(&self) -> String {
        format!("Stop background task: {}", self.task_id)
    }

    pub fn render_args(raw: impl AsRef<str>) -> Result<String, serde_json::Error> {
        let args: Self = serde_json::from_str(raw.as_ref())?;
        Ok(args.title())
    }
}
