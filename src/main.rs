use anyhow::Result;
use colored::Colorize;
use rig::providers::openai;
use rustyline::Config;
use rustyline::error::ReadlineError;

mod agent;
mod commands;
mod completer;
mod config;
mod tools;

use agent::AgentType;
use agent::orchestrate::Orchestrate;
use completer::CommandCompleter;
use config::AppConfig;

#[tokio::main]
async fn main() -> Result<()> {
    let dir = AppConfig::dir().await?;
    let app_config = AppConfig::load_or_setup().await?;

    let mut client_builder = openai::Client::builder().api_key(&app_config.api_key);
    if let Some(base_url) = &app_config.base_url {
        client_builder = client_builder.base_url(base_url);
    }
    let client = client_builder.build()?.completions_api();

    let model_name = app_config.model_name.clone();
    let agent = AgentType::Build.build_agent(&client, &model_name);
    let mut session = Orchestrate::new(agent, "Aries");
    session.set_current_dir();

    let config = Config::builder().auto_add_history(true).build();
    let mut rl = rustyline::Editor::with_config(config)?;
    rl.set_helper(Some(CommandCompleter::new()));

    let history_file = dir.join("history.txt");
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

                if input == "/clear" {
                    session.clear_history();
                    println!("{}", "Chat history cleared.".green());
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
