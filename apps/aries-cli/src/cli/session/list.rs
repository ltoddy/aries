use std::path::PathBuf;

use aries_init::GlobalContext;
use clap::Parser;
use prettytable::{Cell, Row, Table, row};

use crate::Result;
use crate::error::Error;

#[derive(Clone, Debug, Parser)]
#[command(about = "List chat sessions")]
pub struct ListSessionsArgs {
    #[arg(help = "Only list sessions for the specified working directory (absolute path)")]
    cwd: Option<PathBuf>,
}

pub async fn execute(args: ListSessionsArgs, gctx: GlobalContext) -> Result<()> {
    let root_dir = gctx.root_dir();
    let db = aries_persistence::connect(root_dir)
        .await
        .map_err(|source| Error::database("failed to connect local storage:", root_dir, source))?;
    let mut session_repo = aries_persistence::SessionRepository::new(db);

    let sessions = match args.cwd {
        Some(cwd) => session_repo.find_by_cwd(cwd.display().to_string()).await.unwrap_or_default(),
        None => session_repo.find().await.unwrap_or_default(),
    };

    if sessions.is_empty() {
        println!("No sessions found.");
        return Ok(());
    }

    let mut table = Table::new();
    table.add_row(row!["Session ID", "Cwd", "Title", "Created At"]);
    for session in sessions {
        table.add_row(Row::new(vec![
            Cell::new(&session.session_id),
            Cell::new(&session.cwd),
            Cell::new(&clamp(session.title(), 10)),
            Cell::new(&session.created_at.to_string()),
        ]));
    }

    table.printstd();

    Ok(())
}

fn clamp(s: impl Into<String>, max_chars: usize) -> String {
    let s = s.into();
    if s.chars().count() <= max_chars {
        s
    } else {
        let truncated = s.chars().take(max_chars).collect::<String>();
        format!("{}...", truncated)
    }
}
