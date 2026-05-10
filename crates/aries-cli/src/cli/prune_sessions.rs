use std::path::PathBuf;

use anyhow::Context;
use aries_context::GlobalContext;
use clap::Parser;
use futures::stream::{self, StreamExt};

#[derive(Clone, Debug, Parser)]
pub struct PruneSessionsArgs {
    session_ids: Option<Vec<String>>,
    #[arg(long, default_value_t = false)]
    all: bool,
}

pub async fn execute(args: PruneSessionsArgs, gctx: GlobalContext) -> anyhow::Result<()> {
    let db = aries_session::connect(&gctx.config_dir)
        .await
        .with_context(|| format!("Failed to connect local storage: {}", gctx.config_dir.display()))
        .expect("Run `aries init -h` for initialization");
    let mut session_repo = aries_session::persistence::SessionRepository::new(db);

    let PruneSessionsArgs { session_ids, all } = args;

    let sessions = match (session_ids, all) {
        (_, true) => session_repo.find().await?,
        (Some(session_ids), false) => session_repo.find_by_session_id_in(session_ids).await?,
        (None, false) => vec![],
    };

    if sessions.is_empty() {
        eprintln!("No sessions selected for pruning.");
        return Ok(());
    }

    let (session_ids, session_dirs) =
        sessions.into_iter().map(|s| (s.session_id, s.root_dir)).collect::<(Vec<_>, Vec<_>)>();

    session_repo.delete_by_session_id_in(session_ids.clone()).await.with_context(|| {
        format!("Failed to delete sessions from local storage: {:?}", session_ids)
    })?;

    stream::iter(session_dirs)
        .for_each(|dir| async move {
            let _ = tokio::fs::remove_dir_all(PathBuf::from(&dir)).await;
        })
        .await;

    println!("Pruned {} session(s).", session_ids.len());

    Ok(())
}
