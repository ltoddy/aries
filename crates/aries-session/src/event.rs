#[derive(Debug, Clone)]
pub enum StreamEvent {
    Text(String),
    Reasoning(String),
    ToolCall { id: String, name: String, arguments: String },
    ToolResult { id: String, content: String },
    Plan(Vec<PlanEntry>),
    Finish,
}

#[derive(Debug, Clone)]
pub struct PlanEntry {
    pub content: String,
    pub status: PlanEntryStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanEntryStatus {
    Pending,
    InProgress,
    Completed,
}
