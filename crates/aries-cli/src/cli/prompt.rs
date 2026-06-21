use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use aries_context::GlobalContext;
use aries_init::SettingLoader;
use aries_session::SessionRegistry;
use clap::Parser;
use tracing::info_span;

use crate::display::print_agent_event;
use crate::theme::Theme;

#[derive(Parser, Debug, Clone)]
pub struct PromptArgs {
    pub prompt: String,
    #[arg(long)]
    pub session_id: Option<String>,
}

pub async fn execute(args: PromptArgs, gctx: GlobalContext) -> anyhow::Result<()> {
    let session_id = args.session_id.unwrap_or_else(|| nanoid::nanoid!());

    let loader = SettingLoader::new(&gctx.root_dir);
    let setting = loader.load().await?;

    let mut registry = SessionRegistry::new(gctx.clone(), setting).await?;

    aries_logger::init(gctx.root_dir.join("logs"));

    let current_dir = gctx.current_dir.display().to_string();

    let mut session = registry.try_session(&current_dir, &session_id).await?;

    let theme = Theme::default();

    print!("\n{}: ", theme.magenta_text("Aries"));

    let tool_names: Arc<Mutex<HashMap<String, String>>> = Arc::new(Mutex::new(HashMap::new()));
    {
        let _enter = info_span!("prompt", session_id = %session_id).entered();
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
    }
    Ok(())
}
