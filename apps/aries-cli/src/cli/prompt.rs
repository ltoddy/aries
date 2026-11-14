use aries_communicate::AgentEvent;
use aries_init::{GlobalContext, SettingLoader};
use aries_runtime::session::SessionRegistry;
use clap::Parser;
use colored::Colorize;
use tracing::info_span;

use crate::Result;
use crate::display::print_agent_event;
use crate::error::Error;

#[derive(Parser, Debug, Clone)]
#[command(about = "Send a one-shot prompt to the AI")]
pub struct PromptArgs {
    #[arg(help = "The prompt text")]
    pub prompt: String,
    #[arg(long, help = "Override session ID (auto-generated if not provided)")]
    pub session_id: Option<String>,
}

pub async fn execute(args: PromptArgs, gctx: GlobalContext) -> Result<()> {
    let session_id = args.session_id.unwrap_or_else(|| nanoid::nanoid!());

    let loader = SettingLoader::new(gctx.root_dir());
    let setting = loader.load().await.map_err(Error::setting)?;

    let mut registry =
        SessionRegistry::new(gctx.clone(), setting).await.map_err(Error::registry)?;

    aries_logger::init(gctx.root_dir().join("logs"));

    let current_dir = gctx.current_dir();

    let mut session =
        registry.try_session(current_dir, &session_id).await.map_err(Error::registry)?;
    let session_id = session.id();
    let _session_span = info_span!("session", session_id = %session_id).entered();

    print!("{}: ", "Aries".magenta());

    let callback = async |event: AgentEvent| {
        print_agent_event(event);
    };
    session.prompt(&args.prompt, callback).await.map_err(Error::prompt)?;
    Ok(())
}
