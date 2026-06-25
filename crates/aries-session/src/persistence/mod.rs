mod session;
mod token_audit;
mod tool_call;

use std::path::Path;

pub use self::session::{Session, SessionRepository};
pub use self::token_audit::{TokenAudit, TokenAuditRepository};
pub use self::tool_call::{ToolCall, ToolCallRepository};

pub async fn connect(dir: impl AsRef<Path>) -> toasty::Result<toasty::Db> {
    let file_path = dir.as_ref().join("aries.db");
    let url = format!("sqlite://{}", file_path.display());
    let db = toasty::Db::builder()
        .models(toasty::models!(Session, ToolCall, TokenAudit))
        .connect(&url)
        .await?;

    Ok(db)
}

pub async fn migrate(db: &mut toasty::Db) -> toasty::Result<()> {
    db.push_schema().await
}
