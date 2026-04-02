use colored::Colorize;

use crate::config::{AppConfigLoader, setup};

pub const NAME: &str = "/setup";

pub async fn execute() -> anyhow::Result<()> {
    let loader = AppConfigLoader::new().await?;
    let config = setup()?;
    loader.save(&config).await?;
    println!("{}", "Configuration saved successfully!".green());
    Ok(())
}
