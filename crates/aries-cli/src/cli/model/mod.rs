pub mod add;
pub mod current;
pub mod default;
pub mod list;
pub mod rm;

use clap::Subcommand;

use self::add::AddModelArgs;
use self::current::CurrentModelArgs;
use self::default::DefaultModelArgs;
use self::list::ListModelArgs;
use self::rm::RmModelArgs;

#[derive(Subcommand, Debug, Clone)]
pub enum ModelCommand {
    Add(AddModelArgs),
    Current(CurrentModelArgs),
    Default(DefaultModelArgs),
    List(ListModelArgs),
    Rm(RmModelArgs),
}
