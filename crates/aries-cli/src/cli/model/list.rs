use anyhow::Context;
use aries_init::{GlobalContext, SettingLoader};
use clap::Parser;

#[derive(Clone, Debug, Parser)]
pub struct ListModelArgs {}

pub async fn execute(_: ListModelArgs, gctx: GlobalContext) -> anyhow::Result<()> {
    let loader = SettingLoader::new(gctx.root_dir);
    let setting = loader.load().await.with_context(
        || "failed to load setting; run `aries setup` to initialize the configuration",
    )?;

    if setting.models.is_empty() {
        println!("No models configured yet.");
        return Ok(());
    }

    let table = setting.table();
    table.printstd();

    Ok(())
}
