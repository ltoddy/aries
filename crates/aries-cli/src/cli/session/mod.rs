pub mod list;
pub mod prune;
pub mod resume;

use clap::Subcommand;

use self::list::ListSessionsArgs;
use self::prune::PruneSessionsArgs;
use self::resume::ResumeSessionsArgs;

#[derive(Subcommand, Debug, Clone)]
pub enum SessionCommand {
    List(ListSessionsArgs),
    Prune(PruneSessionsArgs),
    Resume(ResumeSessionsArgs),
}
