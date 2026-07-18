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
    #[command(about = "Add a new model configuration")]
    Add(AddModelArgs),
    #[command(about = "Show the currently active model")]
    Current(CurrentModelArgs),
    #[command(about = "Set the default model")]
    Default(DefaultModelArgs),
    #[command(about = "List all configured models")]
    List(ListModelArgs),
    #[command(about = "Remove a model configuration")]
    Rm(RmModelArgs),
}
