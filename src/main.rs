use anyhow::Result;
use colored::Colorize;
use directories::ProjectDirs;
use rig::client::ProviderClient;
use rig::providers::openai;
use rustyline::Config;
use rustyline::error::ReadlineError;

mod agent;
mod completer;
mod tools;

use agent::AgentType;
use agent::session::Session;
use completer::CommandCompleter;

#[tokio::main]
async fn main() -> Result<()> {
    let client = openai::Client::from_env().completions_api();

    let model_name = std::env::var("MODEL_NAME")
        .unwrap_or_else(|_| panic!("The environment variable {} must be set.", "`MODEL_NAME`".cyan()));
    let agent = AgentType::Build.build_agent(&client, &model_name);
    let mut session = Session::new(agent, "Aries");
    session.set_current_dir();

    let config = Config::builder().auto_add_history(true).build();
    let mut rl = rustyline::Editor::with_config(config)?;
    rl.set_helper(Some(CommandCompleter::new()));

    let proj_dirs = ProjectDirs::from("", "", "aries").expect("Failed to determine project directories");
    let config_dir = proj_dirs.config_dir();
    if !config_dir.exists() {
        tokio::fs::create_dir_all(config_dir).await.expect("Failed to create config directory");
    }
    let history_file = config_dir.join("history.txt");

    let _ = rl.load_history(&history_file);

    println!("Welcome to {}! Type '/exit' to quit.", "Aries".green().bold());
    println!("Using model: {}", model_name.cyan());

    loop {
        let readline = rl.readline(">> ");
        match readline {
            Ok(line) => {
                let input = line.trim();
                if input.is_empty() {
                    continue;
                }

                // We enabled auto_add_history, so we don't need this anymore
                // rl.add_history_entry(input)?;

                if input == "/exit" || input == "/quit" {
                    println!("Goodbye!");
                    break;
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
