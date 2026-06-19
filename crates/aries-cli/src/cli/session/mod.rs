pub mod list;
pub mod prune;
pub mod resume;

use clap::Subcommand;

use crate::cli::session::list::ListSessionsArgs;
use crate::cli::session::prune::PruneSessionsArgs;
use crate::cli::session::resume::ResumeSessionsArgs;

#[derive(Subcommand, Debug, Clone)]
pub enum SessionCommand {
    List(ListSessionsArgs),
    Prune(PruneSessionsArgs),
    Resume(ResumeSessionsArgs),
}
