use std::collections::HashMap;
use std::env::current_dir;
use std::sync::{Arc, Mutex};

use aries_init::{GlobalContext, SettingLoader};
use aries_session::SessionRegistry;
use clap::Parser;
use tracing::info_span;

use crate::display::print_agent_event;
use crate::theme::Theme;

#[derive(Parser, Debug, Clone)]
#[command(about = "Send a one-shot prompt to the AI")]
pub struct PromptArgs {
    #[arg(help = "The prompt text")]
    pub prompt: String,
    #[arg(long, help = "Override session ID (auto-generated if not provided)")]
    pub session_id: Option<String>,
}

pub async fn execute(args: PromptArgs, gctx: GlobalContext) -> anyhow::Result<()> {
    let session_id = args.session_id.unwrap_or_else(|| nanoid::nanoid!());

    let loader = SettingLoader::new(&gctx.root_dir);
    let setting = loader.load().await?;

    let mut registry = SessionRegistry::new(gctx.clone(), setting).await?;

    aries_logger::init(gctx.root_dir.join("logs"));

    let current_dir = current_dir().expect("Unable to get current directory");

    let mut session = registry.try_session(current_dir.display().to_string(), &session_id).await?;
    let session_id = session.id();
    let _session_span = info_span!("session", session_id = %session_id).entered();

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
