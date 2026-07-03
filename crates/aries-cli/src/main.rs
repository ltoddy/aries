mod cli;
mod commands;
mod display;
mod input;
mod theme;
mod welcome;

use aries_context::GlobalContext;
use clap::Parser;

use crate::cli::model::{self, ModelCommand};
use crate::cli::session::{self, SessionCommand};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = cli::Args::parse();

    let gctx = GlobalContext::new()?;
    match args.command {
        Some(cli::Subcommands::Setup) => cli::setup::execute(gctx).await,
        Some(cli::Subcommands::Acp) => cli::acp::execute(gctx).await,
        Some(cli::Subcommands::Exec(args)) => cli::exec::execute(args).await,
        Some(cli::Subcommands::Session { command }) => match command {
            SessionCommand::List(args) => session::list::execute(args, gctx).await,
            SessionCommand::Prune(args) => session::prune::execute(args, gctx).await,
            SessionCommand::Resume(args) => session::resume::execute(args, gctx).await,
        },
        Some(cli::Subcommands::Model { command }) => match command {
            ModelCommand::Add(args) => model::add::execute(args, gctx).await,
            ModelCommand::Default(args) => model::default::execute(args, gctx).await,
            ModelCommand::List(args) => model::list::execute(args, gctx).await,
            ModelCommand::Rm(args) => model::rm::execute(args, gctx).await,
        },
        Some(cli::Subcommands::Prompt(args)) => cli::prompt::execute(args, gctx).await,
        _ => cli::run_session(gctx, nanoid::nanoid!()).await,
    }
}
