use std::fmt::{Display, Formatter};

use serde::{Deserialize, Serialize};

use crate::context::{TaskKind, TaskSnapshot, TaskStatus};

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RetrievalStatus {
    NotReady,
    Success,
}

impl Display for RetrievalStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotReady => write!(f, "not_ready"),
            Self::Success => write!(f, "success"),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TaskOutputOutput {
    pub retrieval_status: RetrievalStatus,
    pub task_id: String,
    pub task_type: TaskKind,
    pub status: TaskStatus,
    pub exit_code: Option<i32>,
    pub output: String,
    pub error: String,
}

impl TaskOutputOutput {
    pub fn new(
        retrieval_status: RetrievalStatus,
        task_id: impl Into<String>,
        task_type: TaskKind,
        status: TaskStatus,
        exit_code: Option<i32>,
        output: impl Into<String>,
        error: impl Into<String>,
    ) -> Self {
        let task_id = task_id.into();
        let output = output.into();
        let error = error.into();

        Self { retrieval_status, task_id, task_type, status, exit_code, output, error }
    }

    pub fn render_output(raw: serde_json::Value) -> Result<String, serde_json::Error> {
        let output: Self = serde_json::from_value(raw)?;
        Ok(format!(
            "<retrieval_status>{}</retrieval_status>\n<task_id>{}</task_id>\n<task_type>{:?}</task_type>\n<status>{:?}</status>\n<exit_code>{}</exit_code>\n<output>\n{}\n</output>\n<error>\n{}\n</error>",
            output.retrieval_status,
            output.task_id,
            output.task_type,
            output.status,
            output.exit_code.map(|code| code.to_string()).unwrap_or_else(|| "null".to_owned()),
            output.output,
            output.error
        ))
    }
}

impl From<TaskSnapshot> for TaskOutputOutput {
    fn from(snapshot: TaskSnapshot) -> Self {
        let retrieval_status = if snapshot.status == TaskStatus::Running {
            RetrievalStatus::NotReady
        } else {
            RetrievalStatus::Success
        };

        Self::new(
            retrieval_status,
            snapshot.task_id,
            snapshot.kind,
            snapshot.status,
            snapshot.exit_code,
            snapshot.stdout,
            snapshot.stderr,
        )
    }
}
