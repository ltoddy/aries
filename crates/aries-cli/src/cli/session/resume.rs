use aries_init::GlobalContext;
use clap::Parser;

use crate::cli::run_session;

#[derive(Clone, Debug, Parser)]
pub struct ResumeSessionsArgs {
    session_id: String,
}

pub async fn execute(args: ResumeSessionsArgs, gctx: GlobalContext) -> anyhow::Result<()> {
    let session_id = args.session_id;
    run_session(gctx, session_id).await
}
