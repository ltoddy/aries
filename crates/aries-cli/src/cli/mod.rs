pub mod acp;
pub mod init;
pub mod session;
pub mod setup;

use clap::{Parser, Subcommand};

use crate::cli::init::InitCommand;
use crate::cli::session::SessionCommand;

#[derive(Parser, Debug, Clone)]
pub struct Args {
    #[command(subcommand)]
    pub command: Option<Subcommands>,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Subcommands {
    Acp,
    Init {
        #[command(subcommand)]
        command: InitCommand,
    },
    Session {
        #[command(subcommand)]
        command: SessionCommand,
    },
    Setup,
    Doctor,
}
