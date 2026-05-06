mod acp;
mod cli;
mod commands;
mod display;
mod hook;
mod input;
mod logger;
mod theme;
mod welcome;

use std::time::Instant;

use aries_config::AriesConfigLoader;
use aries_context::GlobalContext;
use aries_session::{NoCb, SessionRegistry};
use clap::Parser;
use rustyline::error::ReadlineError;
use terminal_size::{Width, terminal_size};

use crate::theme::Theme;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let gctx = GlobalContext::new()?;
    let args = cli::Args::parse();

    match args.command {
        Some(cli::Subcommands::Init { command }) => {
            return cli::init::execute(gctx, command).await;
        },
        Some(cli::Subcommands::Setup) => return cli::setup::execute(gctx).await,
        Some(cli::Subcommands::Acp) => return cli::acp::execute(gctx).await,
        _ => {},
    }

    let loader = AriesConfigLoader::new(&gctx.config_dir);
    let config = loader.load_or_setup().await?;

    let mut registry = SessionRegistry::new(gctx.clone(), config.clone()).await?;
    let project = registry.active(&gctx.current_dir).await?;

    let mut session = registry.get_session(project, "main".to_owned()).await?;
    let _guard = logger::init(session.dir());

    let mut reader = input::InputReader::new(session.dir())?;
    welcome::welcome(config.provider(), config.model(), &gctx);

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

                print!("\n{}: ", theme.magenta_text("Aries"));
                let start = Instant::now();
                if let Err(err) =
                    session.prompt(input, None::<NoCb>, hook::DisplayPromptHook::new(theme)).await
                {
                    eprintln!("\n{}: {}", theme.red_text("Error streaming_chunk"), err);
                    continue;
                }

                display_elapsed(start, &theme);
            },
            Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => commands::exit::exit(),
            Err(err) => eprintln!("Error: {:?}", err),
        }
    }
}

fn display_elapsed(start: Instant, theme: &Theme) {
    let elapsed = start.elapsed();
    let terminal_width = terminal_size().map(|(Width(w), _)| w as usize).unwrap_or(80);

    let prefix = "─".repeat(5);
    let time = format!("⏱️  耗时: {:.2}s", elapsed.as_secs_f64());
    let remining_width = terminal_width.saturating_sub(prefix.len() + time.len());
    let line = format!("{}{}{}", "─".repeat(5), time, "─".repeat(remining_width));
    println!("{}\n", theme.dimmed(&line));
}
