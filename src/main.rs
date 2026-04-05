mod args;
mod commands;
mod config;
mod logger;
mod welcome;

use aries_context::GlobalContext;
use aries_core::orchestrate::OrchestrateAgent;
use clap::Parser;
use commands::completer::CommandCompleter;
use rustyline::Config;
use rustyline::error::ReadlineError;

use crate::args::{Args, Subcommands};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let loader = config::AppConfigLoader::new().await?;
    let app_config = loader.load_or_setup().await?;

    logger::init(loader.config_dir());

    let args = Args::parse();

    match args.command {
        Some(Subcommands::Acp) => return aries_acp::execute().await,
        None => {},
    };

    let gctx = GlobalContext::new(app_config).await?;

    let mut orchestrate = OrchestrateAgent::new(gctx.clone())?;

    let config = Config::builder().auto_add_history(true).build();
    let mut rl = rustyline::Editor::with_config(config)?;
    rl.set_helper(Some(CommandCompleter::new()));

    let history_file = gctx.config_dir.join("history.txt");
    let _ = rl.load_history(&history_file);

    welcome::welcome(&gctx.config.model, &gctx);

    let user = whoami::realname().unwrap_or_default();
    loop {
        let readline = rl.readline(format!("{user} >> ").as_str());
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
                    commands::bash::execute(command, &gctx.theme).await;
                    continue;
                }

                if input == commands::save_history::NAME {
                    commands::save_history::execute(orchestrate.chat_history(), &gctx.theme).await;
                    continue;
                }

                if input == commands::clear_history::NAME {
                    orchestrate.clear_history();
                    println!("{}", gctx.theme.green_text("Chat history cleared."));
                    continue;
                }

                if input == commands::setup::NAME {
                    if let Err(e) = commands::setup::execute(&gctx.theme).await {
                        eprintln!("Error: {}", e);
                    }
                    continue;
                }

                if let Err(e) = orchestrate.completion(input).await {
                    eprintln!("Error: {}", e);
                }
            },
            Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => commands::exit::exit(),
            Err(err) => {
                println!("Error: {:?}", err);
                break;
            },
        }
    }
    rl.save_history(&history_file)?;

    Ok(())
}
