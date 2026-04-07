mod args;
mod commands;
mod hook;
mod input;
mod welcome;

use std::time::Instant;

use aries_config::AriesConfigLoader;
use aries_context::GlobalContext;
use aries_session::Session;
use aries_theme::Theme;
use clap::Parser;
use futures::StreamExt;
use rustyline::error::ReadlineError;

use crate::args::{Args, Subcommands};
use crate::hook::DisplayPromptHook;
use crate::input::InputReader;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let gctx = GlobalContext::new()?;

    let loader = AriesConfigLoader::new(&gctx.config_dir);
    let app_config = loader.load_or_setup().await?;

    let args = Args::parse();

    match args.command {
        Some(Subcommands::Acp) => return aries_acp::run(gctx, app_config).await,
        None => {},
    };

    let mut session = Session::new_with_task_hook(
        String::from("main"),
        &gctx,
        app_config.clone(),
        DisplayPromptHook::new(Theme::default()),
    )?;

    let mut rl = InputReader::new(&gctx.config_dir)?;

    welcome::welcome(&app_config.model, &gctx);

    let user = whoami::realname().unwrap_or_default();
    loop {
        let theme = Theme::default();
        let readline = rl.readline(format!("{user} › ").as_str());
        match readline {
            Ok(line) => {
                let input = line.trim();
                if input.is_empty() {
                    continue;
                }

                if input == commands::exit::NAME {
                    commands::exit::exit();
                }

                if let Some(command) = input.strip_prefix(commands::bash::NAME) {
                    commands::bash::execute(command, &theme).await;
                    continue;
                }

                if input == commands::save_history::NAME {
                    commands::save_history::execute(session.history(), &theme).await;
                    continue;
                }

                if input == commands::clear_history::NAME {
                    session.clear_history();
                    println!("{}", theme.green_text("Chat history cleared."));
                    continue;
                }

                if input == commands::setup::NAME {
                    if let Err(e) = commands::setup::execute(&theme, &gctx.config_dir).await {
                        eprintln!("Error: {}", e);
                    }
                    continue;
                }

                let start = Instant::now();
                let theme = Theme::default();
                let stream = session.stream_prompt(input).await;
                tokio::pin!(stream);
                while let Some(chunk) = stream.next().await {
                    if let Err(err) = chunk {
                        eprintln!("\n{}: {}", theme.red_text("Error streaming_chunk"), err)
                    }
                }

                let elapsed = start.elapsed();
                println!("{}", theme.dimmed(&format!("⏱️  耗时: {:.2}s", elapsed.as_secs_f64())));
            },
            Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => commands::exit::exit(),
            Err(err) => {
                println!("Error: {:?}", err);
                break;
            },
        }
    }

    Ok(())
}
