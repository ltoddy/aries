mod args;
mod commands;
mod hook;
mod logger;
mod welcome;

use std::time::Instant;

use aries_config::AriesConfigLoader;
use aries_context::GlobalContext;
use aries_session::Session;
use aries_theme::Theme;
use clap::Parser;
use commands::completer::CommandCompleter;
use futures::StreamExt;
use rig::agent::MultiTurnStreamItem;
use rig::streaming::StreamedAssistantContent;
use rustyline::Config;
use rustyline::error::ReadlineError;

use crate::args::{Args, Subcommands};
use crate::hook::{DisplayPromptHook, display_token_usage};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let gctx = GlobalContext::new()?;
    let _guard = logger::init(&gctx.config_dir);

    let loader = AriesConfigLoader::new(&gctx.config_dir);
    let app_config = loader.load_or_setup().await?;

    let args = Args::parse();

    match args.command {
        Some(Subcommands::Acp) => return aries_acp::run(gctx, app_config).await,
        None => {},
    };

    let init_theme = Theme::default();
    let mut session = Session::new_with_task_hook(
        String::from("main"),
        &gctx,
        app_config.clone(),
        DisplayPromptHook::new(init_theme),
    )?;

    let config = Config::builder().auto_add_history(true).build();
    let mut rl = rustyline::Editor::with_config(config)?;
    rl.set_helper(Some(CommandCompleter::new()));

    let history_file = gctx.config_dir.join("history.txt");
    let _ = rl.load_history(&history_file);

    welcome::welcome(&app_config.model, &gctx);

    let user = whoami::realname().unwrap_or_default();
    loop {
        let theme = Theme::default();
        let readline = rl.readline(format!("{user} › ").as_str());
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
                    commands::save_history::execute(session.history(), &theme).await;
                    continue;
                }

                if input == commands::clear_history::NAME {
                    session.clear_history();
                    println!("{}", theme.green_text("Chat history cleared."));
                    continue;
                }

                if input == commands::setup::NAME {
                    if let Err(e) = commands::setup::execute(&theme, &gctx.config_dir).await {
                        eprintln!("Error: {}", e);
                    }
                    continue;
                }

                if let Err(err) = completion(&mut session, input).await {
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

async fn completion(session: &mut Session<DisplayPromptHook>, input: &str) -> anyhow::Result<()> {
    let start = Instant::now();
    let theme = Theme::default();
    let stream = session.stream_prompt(input).await;
    tokio::pin!(stream);

    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(MultiTurnStreamItem::StreamAssistantItem(StreamedAssistantContent::Reasoning(
                reasoning,
            ))) => display_reasoning(&theme, &reasoning.content),
            Ok(MultiTurnStreamItem::StreamAssistantItem(
                StreamedAssistantContent::ReasoningDelta { id: _, reasoning },
            )) => display_dimmed_text(&theme, &reasoning),
            Ok(MultiTurnStreamItem::FinalResponse(res)) => {
                display_token_usage(&res.usage(), &theme)
            },
            Err(e) => eprintln!("\n{}: {}", theme.red_text("Error streaming_chunk"), e),
            Ok(_) => {},
        }
    }

    let elapsed = start.elapsed();
    println!("{}", theme.dimmed(&format!("⏱️  耗时: {:.2}s", elapsed.as_secs_f64())));

    Ok(())
}

fn display_reasoning(theme: &Theme, content: &[rig::message::ReasoningContent]) {
    let text = content
        .iter()
        .map(|c| match c {
            rig::message::ReasoningContent::Text { text, .. } => text.clone(),
            rig::message::ReasoningContent::Encrypted(s) => s.clone(),
            rig::message::ReasoningContent::Redacted { data } => data.clone(),
            rig::message::ReasoningContent::Summary(s) => s.clone(),
            _ => String::new(),
        })
        .collect::<String>();
    display_dimmed_text(theme, &text);
}

fn display_dimmed_text(theme: &Theme, text: &str) {
    print!("{}", theme.dimmed(text));
    let _ = std::io::Write::flush(&mut std::io::stdout());
}
