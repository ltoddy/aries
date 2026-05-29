use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use aries_config::AriesConfigLoader;
use aries_context::GlobalContext;
use aries_session::SessionRegistry;
use rustyline::error::ReadlineError;
use terminal_size::{Width, terminal_size};

use crate::display::print_stream_item;
use crate::theme::Theme;
use crate::{commands, input, logger, welcome};

pub async fn run_session(gctx: GlobalContext, session_id: impl Into<String>) -> anyhow::Result<()> {
    let loader = AriesConfigLoader::new(&gctx.config_dir);
    let config = loader.load_or_setup().await?;

    let mut registry = SessionRegistry::new(gctx.clone(), config.clone()).await?;

    let current_dir = gctx.current_dir.display().to_string();
    let session_id = session_id.into();

    let mut session = registry.try_session(&current_dir, &session_id).await?;
    let _guard = logger::init(session.dir()).await;

    let mut reader = input::InputReader::new(session.dir())?;
    welcome::welcome(config.provider(), config.model(), &session.id(), &gctx);

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
                let tool_names: Arc<Mutex<HashMap<String, String>>> =
                    Arc::new(Mutex::new(HashMap::new()));
                if let Err(err) = session
                    .prompt(
                        input,
                        Some(|item| {
                            let tool_names = tool_names.clone();
                            async move {
                                if let Ok(mut map) = tool_names.lock() {
                                    print_stream_item(item, theme, &mut map);
                                }
                                Ok(())
                            }
                        }),
                    )
                    .await
                {
                    eprintln!("\n{}: {}", theme.red_text("Error streaming_chunk"), err);
                    continue;
                }

                display_elapsed(start, &theme);
            },
            Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => {
                commands::exit::exit(&session.id())
            },
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
