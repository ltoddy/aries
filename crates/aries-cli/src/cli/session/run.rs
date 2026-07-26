use std::collections::HashMap;
use std::env::current_dir;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use aries_event::AgentEvent;
use aries_extension::mcp::McpDefinition;
use aries_init::{GlobalContext, SettingLoader};
use aries_session::SessionRegistry;
use aries_session::session::SessionArgs;
use rustyline::error::ReadlineError;
use tracing::info_span;

use super::display_elapsed;
use crate::display::print_agent_event;
use crate::theme::Theme;
use crate::{commands, input, welcome};

pub async fn execute(gctx: GlobalContext, bare: bool) -> anyhow::Result<()> {
    let loader = SettingLoader::new(&gctx.root_dir);
    let setting = loader.load().await?;
    let model_config = setting.active_model()?;

    let mut registry = SessionRegistry::new(gctx.clone(), setting.clone()).await?;

    aries_logger::init(gctx.root_dir.join("logs"));

    let current_dir = current_dir().expect("could not determine current directory");

    let session_args = SessionArgs::new(bare);
    let mut session =
        registry.new_session(&current_dir, McpDefinition::empty(), session_args).await?;
    let session_id = session.id();
    let _session_span = info_span!("session", session_id = %session_id).entered();

    let mut reader = input::InputReader::new(session.session_dir())?;
    welcome::welcome(
        model_config.provider().to_string(),
        model_config.model(),
        session.id(),
        &gctx,
        &current_dir,
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

                let callback = async |event: AgentEvent| {
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
