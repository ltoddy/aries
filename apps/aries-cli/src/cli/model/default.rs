use aries_init::{GlobalContext, SettingLoader};
use clap::Parser;
use dialoguer::Select;
use dialoguer::theme::ColorfulTheme;

use crate::Result;
use crate::error::Error;

#[derive(Clone, Debug, Parser)]
#[command(about = "Set the default model")]
pub struct DefaultModelArgs {}

pub async fn execute(_: DefaultModelArgs, gctx: GlobalContext) -> Result<()> {
    let loader = SettingLoader::new(gctx.root_dir());
    let mut setting = loader.load().await.map_err(Error::load_setting)?;

    if setting.models.is_empty() {
        println!("No models configured yet.");
        return Ok(());
    }

    let aliases = setting.aliases();

    let theme = ColorfulTheme::default();
    let alias = aliases[Select::with_theme(&theme)
        .with_prompt("alias")
        .items(&aliases)
        .default(0)
        .interact()
        .map_err(Error::dialog)?]
    .clone();

    println!("Model `{alias}` set as the default.");

    setting.active = alias;
    loader.save(&setting).await.map_err(Error::setting)?;

    Ok(())
}
