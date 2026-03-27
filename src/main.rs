use anyhow::Result;
use colored::Colorize;
use rig::client::ProviderClient;
use rig::completion::Message;
use rig::providers::deepseek;
use rustyline::Config;
use rustyline::error::ReadlineError;

mod agent;
mod completer;
mod tools;

use agent::AgentType;
use agent::runner::run_agent_turn;
use completer::CommandCompleter;

#[tokio::main]
async fn main() -> Result<()> {
    let client = deepseek::Client::from_env();

    let agent = AgentType::Build.build_agent(&client, deepseek::DEEPSEEK_CHAT);

    let mut chat_history: Vec<Message> = vec![];

    let config = Config::builder().auto_add_history(true).build();
    let mut rl = rustyline::Editor::with_config(config)?;
    rl.set_helper(Some(CommandCompleter::new()));

    if rl.load_history("history.txt").is_err() {
        println!("No previous history.");
    }

    println!("Welcome to {}! Type '/exit' to quit.", "Aries".green().bold());
    println!("Using model: {}", deepseek::DEEPSEEK_CHAT.cyan());

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

                if let Err(e) = run_agent_turn(&agent, input, &mut chat_history).await {
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
    rl.save_history("history.txt")?;
    Ok(())
}
