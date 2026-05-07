pub mod acp;
pub mod init;
pub mod setup;

use clap::{Parser, Subcommand};

use crate::cli::init::InitCommand;

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
    Setup,
    Resume {
        session_id: String,
    },
}
