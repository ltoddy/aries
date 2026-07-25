pub mod acp;
pub mod agent;
pub mod exec;
pub mod mcp;
pub mod model;
pub mod prompt;
pub mod session;
pub mod setup;
pub mod skill;
pub mod stats;

use std::collections::HashMap;
use std::env::current_dir;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use aries_init::{GlobalContext, SettingLoader};
use aries_session::SessionRegistry;
use clap::{Parser, Subcommand};
use prompt::PromptArgs;
use rustyline::error::ReadlineError;
use terminal_size::{Width, terminal_size};
use tracing::info_span;

use crate::cli::acp::AcpArgs;
use crate::cli::agent::AgentCommand;
use crate::cli::exec::ExecArgs;
use crate::cli::mcp::McpCommand;
use crate::cli::model::ModelCommand;
use crate::cli::session::SessionCommand;
use crate::cli::skill::SkillCommand;
use crate::cli::stats::StatsCommand;
use crate::display::print_agent_event;
use crate::theme::Theme;
use crate::{commands, input, welcome};

#[derive(Parser, Debug, Clone)]
#[command(about = "Aries: your terminal AI assistant")]
pub struct Args {
    #[command(subcommand)]
    pub command: Option<Subcommands>,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Subcommands {
    #[command(about = "Start an Agent Communication Protocol (ACP) server")]
    Acp(AcpArgs),
    #[command(about = "Manage AI agent configurations")]
    Agent {
        #[command(subcommand)]
        command: AgentCommand,
    },
    #[command(about = "Diagnose and fix common issues")]
    Doctor,
    #[command(about = "Execute a shell command")]
    Exec(ExecArgs),
    #[command(about = "Run git hook integrations")]
    Hook {},
    #[command(about = "Manage MCP (Model Context Protocol) servers")]
    Mcp {
        #[command(subcommand)]
        command: McpCommand,
    },
    #[command(about = "Manage AI model configurations")]
    Model {
        #[command(subcommand)]
        command: ModelCommand,
    },
    #[command(about = "Send a one-shot prompt to the AI")]
    Prompt(PromptArgs),
    #[command(about = "Manage chat sessions")]
    Session {
        #[command(subcommand)]
        command: SessionCommand,
    },
    #[command(about = "Initialize or update Aries configuration")]
    Setup,
    Stats {
        #[command(subcommand)]
        command: StatsCommand,
    },
    #[command(about = "Manage skills (custom tools)")]
    Skill {
        #[command(subcommand)]
        command: SkillCommand,
    },
}

pub async fn run_session(gctx: GlobalContext, session_id: impl Into<String>) -> anyhow::Result<()> {
    let loader = SettingLoader::new(&gctx.root_dir);
    let setting = loader.load().await?;
    let model_config = setting.active_model()?;

    let mut registry = SessionRegistry::new(gctx.clone(), setting.clone()).await?;

    aries_logger::init(gctx.root_dir.join("logs"));

    let current_dir = current_dir().expect("could not determine current directory");
    let session_id = session_id.into();

    let mut session = registry.try_session(current_dir.display().to_string(), &session_id).await?;
    let session_id = session.id();
    let _session_span = info_span!("session", session_id = %session_id).entered();

    let mut reader = input::InputReader::new(session.session_dir())?;
    welcome::welcome(
        model_config.provider().to_string(),
        model_config.model(),
        session.id(),
        &gctx,
        &current_dir,
    );

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
                    commands::execute(input, &theme, &mut session).await;
                    continue;
                }

                print!("\n{}: ", theme.magenta_text("Aries"));
                let start = Instant::now();
                let tool_names: Arc<Mutex<HashMap<String, String>>> =
                    Arc::new(Mutex::new(HashMap::new()));
                if let Err(err) = session
                    .prompt(
                        input,
                        Some(|event| {
                            let tool_names = tool_names.clone();
                            async move {
                                if let Ok(mut map) = tool_names.lock() {
                                    print_agent_event(event, theme, &mut map);
                                }
                            }
                        }),
                    )
                    .await
                {
                    eprintln!("\n{}: {}", theme.red_text("Error"), err);
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
