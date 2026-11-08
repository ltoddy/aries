use clap::Subcommand;

use self::list::ListSkillArgs;

pub mod list;

#[derive(Subcommand, Debug, Clone)]
pub enum SkillCommand {
    #[command(about = "List available skills")]
    List(ListSkillArgs),
}
