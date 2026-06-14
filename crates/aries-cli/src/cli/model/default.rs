use anyhow::{Context, anyhow};
use aries_context::GlobalContext;
use aries_init::SettingLoader;
use clap::Parser;

#[derive(Clone, Debug, Parser)]
pub struct DefaultModelArgs {
    pub alias: String,
}

pub async fn execute(args: DefaultModelArgs, gctx: GlobalContext) -> anyhow::Result<()> {
    let loader = SettingLoader::new(gctx.root_dir);
    let mut setting = loader.load().await.with_context(
        || "failed to load setting; run `aries setup` to initialize the configuration",
    )?;

    if setting.models.is_empty() {
        println!("No models configured yet.");
        return Ok(());
    }

    let DefaultModelArgs { alias } = args;

    if !setting.models.iter().any(|m| m.alias().into() == alias) {
        return Err(anyhow!(""));
    }

    setting.active = alias.clone();

    loader.save(&setting).await?;
    println!("Model `{alias}` set as the default.");

    Ok(())
}
