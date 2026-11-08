use clap::Subcommand;

use self::list::ListHooksArgs;

pub mod list;

#[derive(Subcommand, Debug, Clone)]
pub enum HookCommand {
    #[command(about = "List available hooks")]
    List(ListHooksArgs),
}
