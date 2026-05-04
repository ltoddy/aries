use aries_config::{AriesConfigLoader, setup};
use aries_context::GlobalContext;

use crate::theme::Theme;

pub async fn execute(gctx: GlobalContext) -> anyhow::Result<()> {
    let theme = Theme::default();

    let config = setup()?;
    let loader = AriesConfigLoader::new(gctx.config_dir);
    loader.save(&config).await?;
    println!("{}", theme.green_text("Configuration saved successfully!"));
    Ok(())
}
