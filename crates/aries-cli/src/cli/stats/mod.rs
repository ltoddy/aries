pub mod bash;
pub mod tool;

use clap::Subcommand;

use self::bash::BashArgs;
use self::tool::ToolArgs;

#[derive(Subcommand, Debug, Clone)]
pub enum StatsCommand {
    Bash(BashArgs),
    Tool(ToolArgs),
}
