use clap::Subcommand;

use self::list::ListAgentArgs;

pub mod list;

#[derive(Subcommand, Debug, Clone)]
pub enum AgentCommand {
    #[command(about = "List available agents")]
    List(ListAgentArgs),
}
