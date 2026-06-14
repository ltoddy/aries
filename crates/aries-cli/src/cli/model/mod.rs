pub mod add;
pub mod default;
pub mod list;
pub mod rm;

use clap::Subcommand;

use self::add::AddModelArgs;
use self::default::DefaultModelArgs;
use self::list::ListModelArgs;
use self::rm::RmModelArgs;

#[derive(Subcommand, Debug, Clone)]
pub enum ModelCommand {
    Add(AddModelArgs),
    Default(DefaultModelArgs),
    List(ListModelArgs),
    Rm(RmModelArgs),
}
