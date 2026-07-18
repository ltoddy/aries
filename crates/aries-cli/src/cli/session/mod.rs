pub mod list;
pub mod prune;
pub mod resume;

use clap::Subcommand;

use self::list::ListSessionsArgs;
use self::prune::PruneSessionsArgs;
use self::resume::ResumeSessionsArgs;

#[derive(Subcommand, Debug, Clone)]
pub enum SessionCommand {
    #[command(about = "List chat sessions")]
    List(ListSessionsArgs),
    #[command(about = "Delete old chat sessions")]
    Prune(PruneSessionsArgs),
    #[command(about = "Resume a previous chat session")]
    Resume(ResumeSessionsArgs),
}
