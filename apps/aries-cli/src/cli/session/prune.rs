use aries_init::GlobalContext;
use clap::Parser;
use futures::stream::{self, StreamExt};

use crate::Result;
use crate::error::Error;

#[derive(Clone, Debug, Parser)]
#[command(about = "Delete old chat sessions")]
pub struct PruneSessionsArgs {
    #[arg(help = "Session IDs to delete")]
    session_ids: Option<Vec<String>>,
    #[arg(long, default_value_t = false, help = "Delete all sessions")]
    all: bool,
    #[arg(long, default_value_t = false, help = "Delete all empty sessions")]
    empty: bool,
}

pub async fn execute(args: PruneSessionsArgs, gctx: GlobalContext) -> Result<()> {
    let root_dir = gctx.root_dir();
    let db = aries_persistence::connect(root_dir)
        .await
        .map_err(|source| Error::database("failed to connect local storage:", root_dir, source))?;
    let mut session_repo = aries_persistence::SessionRepository::new(db);

    let PruneSessionsArgs { session_ids, all, empty } = args;

    let sessions = if all {
        session_repo.find().await.map_err(Error::storage)?
    } else if empty {
        session_repo.find_by_title(None).await.map_err(Error::storage)?
    } else if let Some(session_ids) = session_ids {
        session_repo.find_by_session_id_in(session_ids).await.map_err(Error::storage)?
    } else {
        vec![]
    };

    if sessions.is_empty() {
        eprintln!("No sessions selected for pruning.");
        return Ok(());
    }

    let (session_ids, session_dirs) =
        sessions.into_iter().map(|s| (s.session_id, s.root_dir)).collect::<(Vec<_>, Vec<_>)>();

    session_repo
        .delete_by_session_id_in(session_ids.as_slice())
        .await
        .map_err(|source| Error::delete_sessions(session_ids.clone(), source))?;

    stream::iter(session_dirs)
        .for_each(|dir| async move {
            let _ = tokio::fs::remove_dir_all(dir).await;
        })
        .await;

    println!("Pruned {} session(s).", session_ids.len());

    Ok(())
}
