use aries_config::AriesConfigLoader;
use aries_context::GlobalContext;
use aries_session::{NoCb, SessionRegistry};
use clap::Parser;

use crate::theme::Theme;
use crate::{hook, logger};

#[derive(Parser, Debug, Clone)]
pub struct PromptArgs {
    pub prompt: String,
}

pub async fn execute(args: PromptArgs, gctx: GlobalContext) -> anyhow::Result<()> {
    let session_id = nanoid::nanoid!();

    let loader = AriesConfigLoader::new(&gctx.config_dir);
    let config = loader.load_or_setup().await?;

    let mut registry = SessionRegistry::new(gctx.clone(), config.clone()).await?;

    let current_dir = gctx.current_dir.display().to_string();

    let mut session = registry.try_session(&current_dir, &session_id).await?;
    let _guard = logger::init(session.dir()).await;

    let theme = Theme::default();

    print!("\n{}: ", theme.magenta_text("Aries"));
    session.prompt(&args.prompt, None::<NoCb>, hook::DisplayPromptHook::new(theme)).await?;
    Ok(())
}
