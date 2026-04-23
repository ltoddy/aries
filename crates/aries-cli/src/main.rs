mod args;
mod commands;
mod display;
mod hook;
mod input;
mod welcome;

use std::time::Instant;

use aries_config::AriesConfigLoader;
use aries_context::GlobalContext;
use aries_session::{NoCb, Session};
use aries_theme::Theme;
use clap::Parser;
use rustyline::error::ReadlineError;
use terminal_size::{Width, terminal_size};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let gctx = GlobalContext::new()?;

    let loader = AriesConfigLoader::new(&gctx.config_dir);
    let app_config = loader.load_or_setup().await?;

    let args = args::Args::parse();

    match args.command {
        Some(args::Subcommands::Acp) => return aries_acp::run(gctx, app_config).await,
        None => {},
    };

    let mut session = Session::new(
        String::from("main"),
        &gctx,
        app_config.clone(),
        hook::DisplayPromptHook::new(Theme::default()),
    )
    .await?;

    let mut reader = input::InputReader::new(&gctx.config_dir)?;
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

                print!("\n{}: ", theme.magenta_text("Aries"));
                let start = Instant::now();
                if let Err(err) = session.prompt(input, None::<NoCb>).await {
                    eprintln!("\n{}: {}", theme.red_text("Error streaming_chunk"), err);
                    continue;
                }

                let elapsed = start.elapsed();
                let terminal_width = terminal_size().map(|(Width(w), _)| w as usize).unwrap_or(80);

                let prefix = "─".repeat(5);
                let time = format!("⏱️  耗时: {:.2}s", elapsed.as_secs_f64());
                let remining_width = terminal_width.saturating_sub(prefix.len() + time.len());
                let line = format!("{}{}{}", "─".repeat(5), time, "─".repeat(remining_width));
                println!("{}\n", theme.dimmed(&line));
            },
            Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => commands::exit::exit(),
            Err(err) => {
                eprintln!("Error: {:?}", err);
                continue;
            },
        }
    }
}
