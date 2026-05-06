use aries_config::{AriesConfigLoader, setup};
use aries_context::GlobalContext;

use crate::theme::Theme;

pub async fn execute(gctx: GlobalContext) -> anyhow::Result<()> {
    let theme = Theme::default();

    let mut db = aries_session::connect(&gctx.config_dir).await?;
    let _ = aries_session::initalize_tables(&mut db).await;

    let config = setup()?;
    let loader = AriesConfigLoader::new(gctx.config_dir);
    loader.save(&config).await?;
    println!("{}", theme.green_text("Configuration saved successfully!"));
    Ok(())
}
