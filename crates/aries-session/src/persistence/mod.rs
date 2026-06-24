pub mod session;

use std::path::Path;

use aries_track::ToolCall;

pub use crate::persistence::session::{Session, SessionRepository};

pub async fn connect(dir: impl AsRef<Path>) -> toasty::Result<toasty::Db> {
    let file_path = dir.as_ref().join("aries.db");
    let url = format!("sqlite://{}", file_path.display());
    let db = toasty::Db::builder().models(toasty::models!(Session, ToolCall)).connect(&url).await?;

    Ok(db)
}

pub async fn migrate(db: &mut toasty::Db) -> toasty::Result<()> {
    db.push_schema().await
}
