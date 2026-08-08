pub mod bash;
pub mod tool;

use clap::Subcommand;

use self::bash::BashArgs;
use self::tool::ToolArgs;

#[derive(Subcommand, Debug, Clone)]
pub enum StatsCommand {
    #[command(about = "Show bash command usage statistics")]
    Bash(BashArgs),
    #[command(about = "Show tool call statistics")]
    Tool(ToolArgs),
}
