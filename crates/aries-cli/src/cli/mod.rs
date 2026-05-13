pub mod acp;
pub mod init;
pub mod list_sessions;
pub mod prompt;
pub mod prune_sessions;
pub mod resume_session;
pub mod session;
pub mod setup;

use clap::{Parser, Subcommand};
use init::InitCommand;
use list_sessions::ListSessionsArgs;
use prompt::PromptArgs;
use prune_sessions::PruneSessionsArgs;
use resume_session::ResumeSessionsArgs;

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
    Prompt(PromptArgs),
    Session {
        #[command(subcommand)]
        command: SessionCommand,
    },
    Setup,
    Doctor,
}

#[derive(Subcommand, Debug, Clone)]
pub enum SessionCommand {
    List(ListSessionsArgs),
    Prune(PruneSessionsArgs),
    Resume(ResumeSessionsArgs),
}
