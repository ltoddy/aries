mod session;
mod token_audit;
mod tool_call;

use std::path::Path;

use jiff::{Span, Timestamp, Zoned};
pub use session::{Session, SessionRepository};
pub use token_audit::{TokenAudit, TokenAuditRepository};
pub use tool_call::{ToolCall, ToolCallRepository};

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

pub async fn gc(db: toasty::Db) {
    let now = Zoned::now();
    let month_ago = now.saturating_sub(Span::new().days(30));

    let mut session_repo = SessionRepository::new(db.clone());
    if let Ok(sessions) =
        session_repo.find_by_updated_at_less_than(Timestamp::from(&month_ago)).await
    {
        let chunks = sessions.chunks(16).collect::<Vec<_>>();
        for chunk in chunks {
            let session_ids = chunk.iter().map(|s| &s.session_id).collect::<Vec<_>>();
            let _ = session_repo.delete_by_session_id_in(session_ids).await;
        }
    }

    let mut token_audit_repo = TokenAuditRepository::new(db.clone());
    if let Ok(tokens) =
        token_audit_repo.find_by_created_at_less_than(Timestamp::from(&month_ago)).await
    {
        let chunks = tokens.chunks(16).collect::<Vec<_>>();
        for chunk in chunks {
            let ids = chunk.iter().map(|t| t.id).collect::<Vec<_>>();
            let _ = token_audit_repo.delete_by_id_in(ids).await;
        }
    }

    let mut tool_call_repo = ToolCallRepository::new(db.clone());
    if let Ok(tool_calls) =
        tool_call_repo.find_by_created_at_less_than(Timestamp::from(&month_ago)).await
    {
        let chunks = tool_calls.chunks(16).collect::<Vec<_>>();
        for chunk in chunks {
            let ids = chunk.iter().map(|t| t.id).collect::<Vec<_>>();
            let _ = tool_call_repo.delete_by_id_in(ids).await;
        }
    }
}
