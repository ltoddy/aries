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

    let mut reader = InputReader::new(&gctx.config_dir)?;
    welcome::welcome(app_config.provider(), app_config.model(), &gctx);

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
                    commands::execute(input, &theme, &gctx, &mut session).await;
                    continue;
                }

                let start = Instant::now();
                if let Err(err) = session.prompt(input, |_| async { Ok(()) }).await {
                    eprintln!("\n{}: {}", theme.red_text("Error streaming_chunk"), err);
                    continue;
                }

                let elapsed = start.elapsed();
                println!("{}", theme.dimmed(&format!("⏱️  耗时: {:.2}s", elapsed.as_secs_f64())));
            },
            Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => commands::exit::exit(),
            Err(err) => {
                println!("Error: {:?}", err);
                continue;
            },
        }
    }
}
