use std::time::Instant;

use aries_extension::McpDefinition;
use aries_init::{GlobalContext, SettingLoader};
use aries_session::SessionRegistry;
use clap::Parser;
use colored::Colorize;
use rustyline::error::ReadlineError;
use tracing::info_span;

use super::{display_elapsed, prompt_maybe_ask};
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

    let mut reader = input::InputReader::new(&gctx.root_dir)?;
    welcome::welcome(
        model_config.provider().to_string(),
        model_config.model(),
        session.id(),
        &gctx,
        session.current_dir(),
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
