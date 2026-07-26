use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use aries_extension::mcp::McpDefinition;
use aries_init::{GlobalContext, SettingLoader};
use aries_session::SessionRegistry;
use clap::Parser;
use rustyline::error::ReadlineError;
use tracing::info_span;

use super::display_elapsed;
use crate::display::print_agent_event;
use crate::theme::Theme;
use crate::{commands, input, welcome};

#[derive(Clone, Debug, Parser)]
#[command(about = "Resume a previous chat session")]
pub struct ResumeSessionsArgs {
    #[arg(help = "The session ID to resume")]
    session_id: String,
}

pub async fn execute(args: ResumeSessionsArgs, gctx: GlobalContext) -> anyhow::Result<()> {
    let session_id = args.session_id;
    let loader = SettingLoader::new(&gctx.root_dir);
    let setting = loader.load().await?;
    let model_config = setting.active_model()?;

    let mut registry = SessionRegistry::new(gctx.clone(), setting.clone()).await?;

    aries_logger::init(gctx.root_dir.join("logs"));

    let mut session = registry.load_session(&session_id, McpDefinition::empty()).await?;
    let session_id = session.id();
    let _span = info_span!("session", session_id = %session_id).entered();

    let mut reader = input::InputReader::new(session.session_dir())?;
    welcome::welcome(
        model_config.provider().to_string(),
        model_config.model(),
        session.id(),
        &gctx,
        session.current_dir(),
    );

    loop {
        let theme = Theme::default();
        let readline = reader.readline(format!("{} › ", gctx.user).as_str());
        match readline {
            Ok(line) => {
                let input = line.trim();
                if input.is_empty() {
                    continue;
                }

                if input.starts_with('/') {
                    commands::execute(input, &theme, &mut session).await;
                    continue;
                }

                print!("\n{}: ", theme.magenta_text("Aries"));
                let start = Instant::now();
                let tool_names: Arc<Mutex<HashMap<String, String>>> =
                    Arc::new(Mutex::new(HashMap::new()));

                let callback = async |event| {
                    let tool_names = tool_names.clone();
                    if let Ok(mut map) = tool_names.lock() {
                        print_agent_event(event, theme, &mut map);
                    }
                };

                if let Err(err) = session.prompt(input, callback).await {
                    eprintln!("\n{}: {}", theme.red_text("Error"), err);
                    continue;
                }

                display_elapsed(start, &theme);
            },
            Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => {
                commands::exit::exit(&session.id())
            },
            Err(err) => eprintln!("Error: {:?}", err),
        }
    }
}
