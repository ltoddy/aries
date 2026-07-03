use jiff::Timestamp;
use toasty::Db;
use toasty::stmt::IntoExpr;

#[derive(Debug, Clone, toasty::Model)]
#[table = "tool_calls"]
pub struct ToolCall {
    #[key]
    #[auto(increment)]
    id: u64,

    pub session_id: String,
    pub tool_call_id: String,
    #[index]
    pub tool_name: String,
    pub args: String,
    pub duration_ms: Option<u64>,
    pub was_successful: bool,
    #[index]
    #[auto]
    pub created_at: Timestamp,
}

#[derive(Clone)]
pub struct ToolCallRepository {
    db: Db,
}

impl ToolCallRepository {
    pub fn new(db: Db) -> Self {
        Self { db }
    }

    pub async fn create(
        &mut self,
        session_id: impl IntoExpr<String>,
        tool_call_id: impl IntoExpr<String>,
        tool_name: impl IntoExpr<String>,
        args: impl IntoExpr<String>,
        duration_ms: impl IntoExpr<Option<u64>>,
        was_successful: impl IntoExpr<bool>,
    ) -> toasty::Result<ToolCall> {
        ToolCall::create()
            .session_id(session_id)
            .tool_call_id(tool_call_id)
            .tool_name(tool_name)
            .args(args)
            .duration_ms(duration_ms)
            .was_successful(was_successful)
            .exec(&mut self.db)
            .await
    }
}
