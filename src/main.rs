use anyhow::{Context, Result};
use colored::Colorize;
use rustyline::Config;
use rustyline::error::ReadlineError;

mod agent;
mod commands;
mod config;
mod context;
mod tools;

use agent::AgentType;
use agent::orchestrate::Orchestrate;
use commands::completer::CommandCompleter;
use context::GlobalContext;

#[tokio::main]
async fn main() -> Result<()> {
    let current_dir = std::env::current_dir().with_context(|| "无法识别当前目录")?;

    let loader = config::AppConfigLoader::new().await?;
    let app_config = loader.load_or_setup().await?;

    let context = GlobalContext::new(app_config.clone(), current_dir, loader.config_dir().to_path_buf())?;

    let model_name = app_config.model_name.clone();
    let agent = agent::create(&context, AgentType::Build)?;
    let mut session = Orchestrate::new(agent, "Aries", context.clone());

    let config = Config::builder().auto_add_history(true).build();
    let mut rl = rustyline::Editor::with_config(config)?;
    rl.set_helper(Some(CommandCompleter::new()));

    let history_file = context.config_dir.join("history.txt");
    let _ = rl.load_history(&history_file);

    println!("Welcome to {}! Type '{}' to quit.", commands::exit::NAME, "Aries".green().bold());
    println!("Using model: {}", model_name.cyan());

    loop {
        let readline = rl.readline(">> ");
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
                    commands::bash::execute(command).await;
                    continue;
                }

                if input == commands::save_history::NAME {
                    commands::save_history::execute(session.chat_history()).await;
                    continue;
                }

                if input == "/clear" {
                    session.clear_history();
                    println!("{}", "Chat history cleared.".green());
                    continue;
                }

                if input == commands::setup::NAME {
                    if let Err(e) = commands::setup::execute().await {
                        eprintln!("Error: {}", e);
                    }
                    continue;
                }

                if let Err(e) = session.completion(input).await {
                    eprintln!("Error: {}", e);
                }
            },
            Err(ReadlineError::Interrupted) => {
                println!("CTRL-C");
                break;
            },
            Err(ReadlineError::Eof) => {
                println!("CTRL-D");
                break;
            },
            Err(err) => {
                println!("Error: {:?}", err);
                break;
            },
        }
    }
    rl.save_history(&history_file)?;

    Ok(())
}
