use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use aries_config::AriesConfigLoader;
use aries_context::GlobalContext;
use aries_session::SessionRegistry;
use clap::Parser;

use crate::display::print_agent_event;
use crate::logger;
use crate::theme::Theme;

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

    let tool_names: Arc<Mutex<HashMap<String, String>>> = Arc::new(Mutex::new(HashMap::new()));
    session
        .prompt(
            &args.prompt,
            Some(|event| {
                let tool_names = tool_names.clone();
                async move {
                    if let Ok(mut map) = tool_names.lock() {
                        print_agent_event(event, theme, &mut map);
                    }
                }
            }),
        )
        .await?;
    Ok(())
}
