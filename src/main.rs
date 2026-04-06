mod args;
mod commands;
mod display;
mod logger;
mod welcome;

use std::io::Write;
use std::time::Instant;

use aries_config::AriesConfigLoader;
use aries_context::GlobalContext;
use aries_core::orchestrate::OrchestrateAgent;
use aries_theme::Theme;
use clap::Parser;
use colored::Colorize;
use commands::completer::CommandCompleter;
use futures::StreamExt;
use rig::agent::{MultiTurnStreamItem, Text};
use rig::streaming::{StreamedAssistantContent, StreamedUserContent};
use rustyline::Config;
use rustyline::error::ReadlineError;

use crate::args::{Args, Subcommands};
use crate::display::{display_token_usage, display_tool_call, display_tool_result};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let gctx = GlobalContext::new()?;
    let _guard = logger::init(&gctx.config_dir);

    let loader = AriesConfigLoader::new(&gctx.config_dir);
    let app_config = loader.load_or_setup().await?;

    let args = Args::parse();

    let mut orchestrate = OrchestrateAgent::new(gctx.clone(), app_config.clone())?;

    match args.command {
        Some(Subcommands::Acp) => return aries_acp::run(gctx, orchestrate).await,
        None => {},
    };

    let config = Config::builder().auto_add_history(true).build();
    let mut rl = rustyline::Editor::with_config(config)?;
    rl.set_helper(Some(CommandCompleter::new()));

    let history_file = gctx.config_dir.join("history.txt");
    let _ = rl.load_history(&history_file);

    welcome::welcome(&app_config.model, &gctx);

    let user = whoami::realname().unwrap_or_default();
    loop {
        let theme = Theme::default();
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
                    commands::bash::execute(command, &theme).await;
                    continue;
                }

                if input == commands::save_history::NAME {
                    commands::save_history::execute(orchestrate.chat_history_ref(), &theme).await;
                    continue;
                }

                if input == commands::clear_history::NAME {
                    orchestrate.clear_history(1);
                    println!("{}", theme.green_text("Chat history cleared."));
                    continue;
                }

                if input == commands::setup::NAME {
                    if let Err(e) = commands::setup::execute(&theme, &gctx.config_dir).await {
                        eprintln!("Error: {}", e);
                    }
                    continue;
                }

                if let Err(err) = completion(&mut orchestrate, input).await {
                    eprintln!("Error: {}", err);
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

async fn completion(orchestrate: &mut OrchestrateAgent, input: &str) -> anyhow::Result<()> {
    let start = Instant::now();
    let theme = Theme::default();

    println!("{}:", theme.green_text(&orchestrate.name).bold());

    let stream = orchestrate.stream_prompt(input).await;
    tokio::pin!(stream);
    let mut active_tools: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();

    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(MultiTurnStreamItem::StreamAssistantItem(StreamedAssistantContent::Text(
                Text { text },
            ))) => {
                print!("{}", text);
                let _ = std::io::stdout().flush();
            },
            Ok(MultiTurnStreamItem::StreamAssistantItem(StreamedAssistantContent::Reasoning(
                reasoning,
            ))) => {
                let text = reasoning
                    .content
                    .iter()
                    .map(|c| match c {
                        rig::message::ReasoningContent::Text { text, .. } => text.clone(),
                        rig::message::ReasoningContent::Encrypted(s) => s.clone(),
                        rig::message::ReasoningContent::Redacted { data } => data.clone(),
                        rig::message::ReasoningContent::Summary(s) => s.clone(),
                        _ => String::new(),
                    })
                    .collect::<String>();
                print!("{}", theme.dimmed(&text));
                let _ = std::io::stdout().flush();
            },
            Ok(MultiTurnStreamItem::StreamAssistantItem(
                StreamedAssistantContent::ReasoningDelta { id: _, reasoning },
            )) => {
                print!("{}", theme.dimmed(&reasoning));
                let _ = std::io::stdout().flush();
            },
            Ok(MultiTurnStreamItem::StreamAssistantItem(StreamedAssistantContent::ToolCall {
                tool_call,
                ..
            })) => {
                active_tools.insert(tool_call.id.clone(), tool_call.function.name.clone());
                display_tool_call(&tool_call.function.name, &tool_call.function.arguments, &theme);
            },
            Ok(MultiTurnStreamItem::StreamUserItem(StreamedUserContent::ToolResult {
                tool_result,
                ..
            })) => {
                let tool_name =
                    active_tools.get(&tool_result.id).cloned().unwrap_or_else(String::new);
                let json_str =
                    serde_json::to_string(&tool_result).unwrap_or_else(|_| String::new());

                display_tool_result(&tool_name, &json_str, &theme);
            },
            Ok(MultiTurnStreamItem::FinalResponse(res)) => {
                display_token_usage(&res.usage(), &theme);
            },
            Err(e) => eprintln!("\n{}: {}", theme.red_text("Error streaming_chunk"), e),
            Ok(_) => {},
        }
    }
    println!();

    let elapsed = start.elapsed();
    println!("{}", theme.dimmed(&format!("⏱️  耗时: {:.2}s", elapsed.as_secs_f64())));

    // TODO
    // let messages = orchestrate.chat_history();
    // if let Ok(Some(summary)) =
    // orchestrate.compaction_agent.compact(messages).await {     orchestrate.
    // clear_history(1);     orchestrate.history.
    // push(Message::assistant(summary)); }

    Ok(())
}
