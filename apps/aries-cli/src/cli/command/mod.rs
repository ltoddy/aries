use clap::Subcommand;

use self::list::ListCommandArgs;

pub mod list;

#[derive(Subcommand, Debug, Clone)]
pub enum CommandCommand {
    #[command(about = "List available commands")]
    List(ListCommandArgs),
}
