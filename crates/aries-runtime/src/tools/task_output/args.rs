use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TaskOutputArgs {
    pub task_id: String,
    #[serde(default = "default_block")]
    pub block: bool,
}

impl TaskOutputArgs {
    pub fn title(&self) -> String {
        format!("Read background task output: {}", self.task_id)
    }

    pub fn render_args(raw: &str) -> Result<String, serde_json::Error> {
        let args: Self = serde_json::from_str(raw)?;
        Ok(args.title())
    }
}

fn default_block() -> bool {
    true
}
