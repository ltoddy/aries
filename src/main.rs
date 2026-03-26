use std::io::Write;

use anyhow::Result;
use colored::Colorize;
use futures::StreamExt;
use rig::client::{CompletionClient, ProviderClient};
use rig::completion::Message;
use rig::message::Text;
use rig::providers::deepseek;
use rig::streaming::{StreamedAssistantContent, StreamingPrompt};
use rustyline::Config;
use rustyline::error::ReadlineError;

mod completer;
use completer::CommandCompleter;
use rig::agent::MultiTurnStreamItem;

#[tokio::main]
async fn main() -> Result<()> {
    let client = deepseek::Client::from_env();

    let agent =
        client.agent(deepseek::DEEPSEEK_CHAT).preamble("You are Aries, a helpful terminal AI assistant.").build();

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

                // Call LLM using rig-core streaming
                let mut stream = agent.stream_prompt(input).with_history(chat_history.clone()).await;

                print!("{}: ", "Aries".green().bold());
                let mut full_response = String::new();

                while let Some(chunk) = stream.next().await {
                    match chunk {
                        Ok(MultiTurnStreamItem::StreamAssistantItem(StreamedAssistantContent::Text(Text { text }))) => {
                            print!("{}", text);
                            std::io::stdout().flush().unwrap_or_default();
                            full_response.push_str(&text);
                        },
                        Ok(MultiTurnStreamItem::FinalResponse(res)) => {
                            if let Some(history) = res.history() {
                                chat_history = history.to_vec();
                            }
                        },
                        Err(e) => eprintln!("\n{}: {}", "Error streaming chunk".red(), e),
                        _ => {},
                    }
                }
                println!();
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
