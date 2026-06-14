use anyhow::Context;
use aries_context::GlobalContext;
use aries_init::SettingLoader;
use clap::Parser;
use dialoguer::Select;
use dialoguer::theme::ColorfulTheme;

#[derive(Clone, Debug, Parser)]
pub struct RmModelArgs {}

pub async fn execute(_: RmModelArgs, gctx: GlobalContext) -> anyhow::Result<()> {
    let loader = SettingLoader::new(gctx.root_dir);
    let mut setting = loader.load().await.with_context(
        || "failed to load setting; run `aries setup` to initialize the configuration",
    )?;

    if setting.models.is_empty() {
        println!("No models configured yet.");
        return Ok(());
    }

    println!("Current models:");
    let table = setting.table();
    table.printstd();

    let aliases = setting.aliases();
    let items = aliases.iter().map(|a| a.as_str()).collect::<Vec<_>>();

    let theme = ColorfulTheme::default();
    let alias = aliases
        [Select::with_theme(&theme).with_prompt("alias").items(items).default(0).interact()?]
    .clone();

    if setting.active == alias {
        println!(
            "model `{alias}` is currently active and cannot be removed; only inactive models can be removed."
        );
        return Ok(());
    }

    if !setting.models.iter().any(|m| m.alias().into() == alias) {
        println!("no model found with alias `{alias}`.");
        return Ok(());
    }

    setting.models =
        setting.models.into_iter().filter(|m| m.alias().into() != alias).collect::<Vec<_>>();

    loader.save(&setting).await?;

    println!("model `{alias}` removed.");
    Ok(())
}
