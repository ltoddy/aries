use aries_context::Theme;

use crate::config::{AppConfigLoader, setup};

pub const NAME: &str = "/setup";

pub async fn execute(theme: &Theme) -> anyhow::Result<()> {
    let loader = AppConfigLoader::new().await?;
    let config = setup()?;
    loader.save(&config).await?;
    println!("{}", theme.green_text("Configuration saved successfully!"));
    Ok(())
}
