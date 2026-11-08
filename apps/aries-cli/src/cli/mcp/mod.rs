use clap::Subcommand;

use self::list::ListMcpArgs;

pub mod list;

#[derive(Subcommand, Debug, Clone)]
pub enum McpCommand {
    #[command(about = "List configured MCP servers")]
    List(ListMcpArgs),
}
