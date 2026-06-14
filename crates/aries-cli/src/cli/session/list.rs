use std::path::PathBuf;

use anyhow::Context;
use aries_context::GlobalContext;
use clap::Parser;
use prettytable::{Cell, Row, Table, row};

#[derive(Clone, Debug, Parser)]
pub struct ListSessionsArgs {
    #[arg(help = "Only list sessions for the specified working directory (absolute path)")]
    cwd: Option<PathBuf>,
}

pub async fn execute(args: ListSessionsArgs, gctx: GlobalContext) -> anyhow::Result<()> {
    let db = aries_session::connect(&gctx.root_dir)
        .await
        .with_context(|| format!("Failed to connect local storage: {}", gctx.root_dir.display()))
        .expect("Run `aries init -h` for initialization");
    let mut session_repo = aries_session::persistence::SessionRepository::new(db);

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
    sessions
        .into_iter()
        .map(|s| {
            Row::new(vec![
                Cell::new(s.session_id.as_str()),
                Cell::new(s.cwd.as_str()),
                Cell::new(s.title.unwrap_or_default().as_str()),
                Cell::new(s.created_at.to_string().as_str()),
            ])
        })
        .for_each(|row| {
            table.add_row(row);
        });

    table.printstd();

    Ok(())
}
