use std::path::Path;

use aries_config::{AriesConfigLoader, setup};
use aries_theme::Theme;

pub const NAME: &str = "/setup";

pub async fn execute(theme: &Theme, config_dir: &Path) -> anyhow::Result<()> {
    let loader = AriesConfigLoader::new(config_dir);
    let config = setup()?;
    loader.save(&config).await?;
    println!("{}", theme.green_text("Configuration saved successfully!"));
    Ok(())
}
