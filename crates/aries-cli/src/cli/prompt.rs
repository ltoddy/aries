use std::collections::HashMap;
use std::env::current_dir;
use std::sync::{Arc, Mutex};

use aries_event::AgentEvent;
use aries_init::{GlobalContext, SettingLoader};
use aries_session::SessionRegistry;
use aries_session::session::SessionArgs;
use clap::Parser;
use colored::Colorize;
use tracing::info_span;

use crate::display::print_agent_event;

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

    let session_args = SessionArgs::default();
    let mut session = registry.try_session(current_dir, &session_id, session_args).await?;
    let session_id = session.id();
    let _session_span = info_span!("session", session_id = %session_id).entered();

    print!("\n{}: ", "Aries".magenta());

    let tool_names: Arc<Mutex<HashMap<String, String>>> = Arc::new(Mutex::new(HashMap::new()));
    let callback = async |event: AgentEvent| {
        let tool_names = tool_names.clone();
        if let Ok(mut map) = tool_names.lock() {
            print_agent_event(event, &mut map);
        }
    };
    session.prompt(&args.prompt, callback).await?;
    Ok(())
}
