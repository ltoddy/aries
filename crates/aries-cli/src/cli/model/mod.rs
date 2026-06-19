pub mod add;
pub mod default;
pub mod list;
pub mod rm;

use clap::Subcommand;

use crate::cli::model::add::AddModelArgs;
use crate::cli::model::default::DefaultModelArgs;
use crate::cli::model::list::ListModelArgs;
use crate::cli::model::rm::RmModelArgs;

#[derive(Subcommand, Debug, Clone)]
pub enum ModelCommand {
    Add(AddModelArgs),
    Default(DefaultModelArgs),
    List(ListModelArgs),
    Rm(RmModelArgs),
}
