mod cli;
mod commands;
mod display;
mod hook;
mod input;
mod logger;
mod theme;
mod welcome;

use aries_context::GlobalContext;
use clap::Parser;

use crate::cli::SessionCommand;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = cli::Args::parse();

    let gctx = GlobalContext::new()?;
    match args.command {
        Some(cli::Subcommands::Init { command }) => cli::init::execute(gctx, command).await,
        Some(cli::Subcommands::Setup) => cli::setup::execute(gctx).await,
        Some(cli::Subcommands::Acp) => cli::acp::execute(gctx).await,
        Some(cli::Subcommands::Session { command }) => match command {
            SessionCommand::List(args) => cli::list_sessions::execute(args, gctx).await,
            SessionCommand::Prune(args) => cli::prune_sessions::execute(args, gctx).await,
            SessionCommand::Resume(args) => cli::resume_session::execute(args, gctx).await,
        },
        Some(cli::Subcommands::Prompt(args)) => cli::prompt::execute(args, gctx).await,
        _ => cli::session::run_session(gctx, nanoid::nanoid!()).await,
    }
}
