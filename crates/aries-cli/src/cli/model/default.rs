use anyhow::Context;
use aries_init::{GlobalContext, SettingLoader};
use clap::Parser;
use dialoguer::Select;
use dialoguer::theme::ColorfulTheme;

#[derive(Clone, Debug, Parser)]
#[command(about = "Set the default model")]
pub struct DefaultModelArgs {}

pub async fn execute(_: DefaultModelArgs, gctx: GlobalContext) -> anyhow::Result<()> {
    let loader = SettingLoader::new(gctx.root_dir);
    let mut setting = loader.load().await.with_context(
        || "failed to load setting; run `aries setup` to initialize the configuration",
    )?;

    if setting.models.is_empty() {
        println!("No models configured yet.");
        return Ok(());
    }

    let aliases = setting.aliases();
    let items = aliases.iter().map(|a| a.as_str()).collect::<Vec<_>>();

    let theme = ColorfulTheme::default();
    let alias = aliases
        [Select::with_theme(&theme).with_prompt("alias").items(items).default(0).interact()?]
    .clone();

    println!("Model `{alias}` set as the default.");

    setting.active = alias;
    loader.save(&setting).await?;

    Ok(())
}
