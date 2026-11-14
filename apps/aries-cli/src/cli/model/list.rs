use aries_init::{GlobalContext, SettingLoader};
use clap::Parser;

use crate::Result;
use crate::error::Error;

#[derive(Clone, Debug, Parser)]
#[command(about = "List all configured models")]
pub struct ListModelArgs {}

pub async fn execute(_: ListModelArgs, gctx: GlobalContext) -> Result<()> {
    let loader = SettingLoader::new(gctx.root_dir());
    let setting = loader.load().await.map_err(Error::load_setting)?;

    if setting.models.is_empty() {
        println!("No models configured yet.");
        return Ok(());
    }

    let table = setting.table();
    table.printstd();

    Ok(())
}
