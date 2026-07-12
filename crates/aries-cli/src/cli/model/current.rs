use anyhow::Context;
use aries_init::{GlobalContext, SettingLoader};
use clap::Parser;

#[derive(Clone, Debug, Parser)]
pub struct CurrentModelArgs {}

pub async fn execute(_: CurrentModelArgs, gctx: GlobalContext) -> anyhow::Result<()> {
    let loader = SettingLoader::new(gctx.root_dir);
    let setting = loader.load().await.with_context(
        || "failed to load setting; run `aries setup` to initialize the configuration",
    )?;

    let current = setting.active_model()?;
    println!("{}", current.model());
    Ok(())
}
