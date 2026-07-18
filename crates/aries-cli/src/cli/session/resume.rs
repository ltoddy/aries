use aries_init::GlobalContext;
use clap::Parser;

use crate::cli::run_session;

#[derive(Clone, Debug, Parser)]
#[command(about = "Resume a previous chat session")]
pub struct ResumeSessionsArgs {
    #[arg(help = "The session ID to resume")]
    session_id: String,
}

pub async fn execute(args: ResumeSessionsArgs, gctx: GlobalContext) -> anyhow::Result<()> {
    let session_id = args.session_id;
    run_session(gctx, session_id).await
}
