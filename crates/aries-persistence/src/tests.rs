// This file contains tests generated with AI assistance.

use jiff::{Span, Timestamp, Zoned};
use toasty::Db;

use super::{
    Session, SessionRepository, TokenAudit, TokenAuditRepository, ToolCall, ToolCallRepository,
    migrate,
};

async fn memory_db() -> toasty::Result<Db> {
    let driver = toasty_driver_sqlite::Sqlite::in_memory();
    let mut db =
        Db::builder().models(toasty::models!(Session, ToolCall, TokenAudit)).build(driver).await?;
    migrate(&mut db).await?;

    Ok(db)
}

fn past() -> Timestamp {
    Timestamp::from(&Zoned::now().saturating_sub(Span::new().days(1)))
}

fn future() -> Timestamp {
    Timestamp::from(&Zoned::now().saturating_add(Span::new().days(1)))
}

#[tokio::test]
async fn session_repository_uses_memory_database() {
    let db = memory_db().await.unwrap();
    let mut repo = SessionRepository::new(db);

    let first = repo.create("session-1", "/workspace", "/", "/tmp/session-1.jsonl").await.unwrap();
    let second = repo.create("session-2", "/workspace", "/", "/tmp/session-2.jsonl").await.unwrap();

    assert_eq!(first.session_id, "session-1");
    assert_eq!(second.session_id, "session-2");

    let sessions = repo.find().await.unwrap();
    assert_eq!(sessions.len(), 2);

    let sessions = repo.find_by_cwd("/workspace").await.unwrap();
    assert_eq!(sessions.len(), 2);

    let session = repo.find_last_by_session_id("session-1").await.unwrap();
    assert_eq!(session.transcript_path, "/tmp/session-1.jsonl");

    let sessions = repo
        .find_by_session_id_in(vec!["session-1".to_owned(), "session-2".to_owned()])
        .await
        .unwrap();
    assert_eq!(sessions.len(), 2);

    let sessions = repo.find_by_updated_at_less_than(future()).await.unwrap();
    assert_eq!(sessions.len(), 2);

    let sessions = repo.find_by_null_title().await.unwrap();
    assert_eq!(sessions.len(), 2);

    repo.update_title_by_session_id("session-1", Some("title".to_owned())).await.unwrap();

    let session = repo.find_last_by_session_id("session-1").await.unwrap();
    assert_eq!(session.title, Some("title".to_owned()));

    let sessions = repo.find_by_null_title().await.unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].session_id, "session-2");

    repo.delete_by_session_id("session-1").await.unwrap();
    let sessions = repo.find().await.unwrap();
    assert_eq!(sessions.len(), 1);

    repo.delete_by_session_id_in(vec!["session-2".to_owned()]).await.unwrap();
    let sessions = repo.find().await.unwrap();
    assert!(sessions.is_empty());
}

#[tokio::test]
async fn tool_call_repository_uses_memory_database() {
    let db = memory_db().await.unwrap();
    let mut repo = ToolCallRepository::new(db);

    let tool_call = repo
        .create("session-1", "tool-call-1", "Read", r#"{"file_path":"Cargo.toml"}"#, Some(10), true)
        .await
        .unwrap();

    assert_eq!(tool_call.session_id, "session-1");
    assert_eq!(tool_call.tool_call_id, "tool-call-1");
    assert_eq!(tool_call.tool_name, "Read");
    assert_eq!(tool_call.duration_ms, Some(10));
    assert!(tool_call.was_successful);

    let tool_calls = repo.find_by_created_at_greater_than(past()).await.unwrap();
    assert_eq!(tool_calls.len(), 1);

    let tool_calls = repo.find_by_created_at_less_than(future()).await.unwrap();
    assert_eq!(tool_calls.len(), 1);

    repo.delete_by_id_in(vec![tool_call.id]).await.unwrap();
    let tool_calls = repo.find_by_created_at_less_than(future()).await.unwrap();
    assert!(tool_calls.is_empty());
}

#[tokio::test]
async fn token_audit_repository_uses_memory_database() {
    let db = memory_db().await.unwrap();
    let mut repo = TokenAuditRepository::new(db);

    let audit = repo.create("compact", 100, 60).await.unwrap();
    assert_eq!(audit.command, "compact");
    assert_eq!(audit.original_tokens, 100);
    assert_eq!(audit.optimized_tokens, 60);
    assert_eq!(audit.saved_tokens, 40);
    assert_eq!(audit.savings_percent, 40.0);

    let zero = repo.create("empty", 0, 10).await.unwrap();
    assert_eq!(zero.saved_tokens, 0);
    assert_eq!(zero.savings_percent, 0.0);

    let audits = repo.find_by_created_at_less_than(future()).await.unwrap();
    assert_eq!(audits.len(), 2);

    repo.delete_by_id_in(vec![audit.id, zero.id]).await.unwrap();
    let audits = repo.find_by_created_at_less_than(future()).await.unwrap();
    assert!(audits.is_empty());
}
