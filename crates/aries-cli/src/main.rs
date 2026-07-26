mod cli;
mod commands;
mod display;
mod input;
mod text;
mod theme;
mod welcome;

use aries_init::GlobalContext;
use clap::Parser;

use crate::cli::agent::{self, AgentCommand};
use crate::cli::mcp::{self, McpCommand};
use crate::cli::model::{self, ModelCommand};
use crate::cli::session::{self, SessionCommand};
use crate::cli::skill::{self, SkillCommand};
use crate::cli::stats::StatsCommand;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = cli::Args::parse();

    let gctx = GlobalContext::new().await;
    aries_init::init(&gctx.root_dir).await;

    match args.command {
        Some(cli::Subcommands::Acp(args)) => cli::acp::execute(args, gctx).await,
        Some(cli::Subcommands::Agent { command }) => match command {
            AgentCommand::List(args) => agent::list::execute(args, gctx).await,
        },
        Some(cli::Subcommands::Doctor) => {
            todo!()
        },
        Some(cli::Subcommands::Exec(args)) => cli::exec::execute(args).await,
        Some(cli::Subcommands::Hook { .. }) => {
            todo!()
        },
        Some(cli::Subcommands::Mcp { command }) => match command {
            McpCommand::List(args) => mcp::list::execute(args, gctx).await,
        },
        Some(cli::Subcommands::Model { command }) => match command {
            ModelCommand::Add(args) => model::add::execute(args, gctx).await,
            ModelCommand::Current(args) => model::current::execute(args, gctx).await,
            ModelCommand::Default(args) => model::default::execute(args, gctx).await,
            ModelCommand::List(args) => model::list::execute(args, gctx).await,
            ModelCommand::Rm(args) => model::rm::execute(args, gctx).await,
        },
        Some(cli::Subcommands::Prompt(args)) => cli::prompt::execute(args, gctx).await,
        Some(cli::Subcommands::Session { command }) => match command {
            SessionCommand::List(args) => session::list::execute(args, gctx).await,
            SessionCommand::Prune(args) => session::prune::execute(args, gctx).await,
            SessionCommand::Resume(args) => session::resume::execute(args, gctx).await,
        },
        Some(cli::Subcommands::Setup) => cli::setup::execute(gctx).await,
        Some(cli::Subcommands::Stats { command }) => match command {
            StatsCommand::Bash(args) => cli::stats::bash::execute(gctx, args).await,
            StatsCommand::Tool(args) => cli::stats::tool::execute(gctx, args).await,
        },
        Some(cli::Subcommands::Skill { command }) => match command {
            SkillCommand::List(args) => skill::list::execute(args, gctx).await,
        },
        _ => session::run::execute(gctx, args.bare).await,
    }
}
