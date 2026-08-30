use std::env::current_dir;
use std::time::Instant;

use aries_extension::McpDefinition;
use aries_init::{GlobalContext, SettingLoader};
use aries_session::{SessionArgs, SessionRegistry};
use colored::Colorize;
use rustyline::error::ReadlineError;
use tracing::info_span;

use super::{display_elapsed, prompt_maybe_ask};
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

    let mut reader = input::InputReader::new(&gctx.root_dir)?;
    welcome::welcome(
        model_config.provider().to_string(),
        model_config.model(),
        session.id(),
        &gctx,
        &current_dir,
    );

    loop {
        let readline = reader.readline(format!("{} › ", gctx.user).as_str());
        match readline {
            Ok(line) => {
                reader.save_history();
                let input = line.trim();
                if input.is_empty() {
                    continue;
                }

                print!("\n{}: ", "Aries".magenta());
                let start = Instant::now();

                if let Err(err) = prompt_maybe_ask(&mut session, input).await {
                    eprintln!("\n{}: {}", "Error".red(), err);
                    continue;
                }

                display_elapsed(start);
            },
            Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => {
                commands::exit::exit(&session.id())
            },
            Err(err) => eprintln!("Error: {:?}", err),
        }
    }
}
