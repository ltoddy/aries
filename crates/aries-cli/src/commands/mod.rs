use aries_context::GlobalContext;
use aries_session::Session;
use clap::{Parser, Subcommand};
use rig::agent::PromptHook;
use rig::providers::{azure, openai};

use crate::theme::Theme;

pub mod clear_history;
pub mod compact;
pub mod completer;
pub mod exit;
pub mod save_history;
pub mod setup;
pub mod shell;

#[derive(Parser)]
#[command(name = "aries")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Exit Aries
    Exit,
    /// Run a shell command
    Shell {
        /// The command to execute
        #[arg(trailing_var_arg = true)]
        command: Vec<String>,
    },
    /// Open configuration setup
    Setup,
    /// Save chat history to file
    SaveHistory,
    /// Clear chat history
    ClearHistory,
    /// Force compact conversation context
    Compact,
}

impl Command {
    pub fn names() -> &'static [(&'static str, &'static str)] {
        &[
            ("exit", "Exit Aries"),
            ("shell", "Run a shell command"),
            ("setup", "Open configuration setup"),
            ("save-history", "Save chat history to file"),
            ("clear-history", "Clear chat history"),
            ("compact", "Force compact conversation context"),
        ]
    }
}

pub async fn execute<P>(input: &str, theme: &Theme, gctx: &GlobalContext, session: &mut Session<P>)
where
    P: PromptHook<openai::CompletionModel> + PromptHook<azure::CompletionModel> + 'static,
{
    let input = input.strip_prefix('/').unwrap_or(input);
    let mut args = vec![String::from(env!("CARGO_PKG_NAME"))];

    if let Some(rest) = input.strip_prefix("help") {
        let rest = rest.trim();
        if rest.is_empty() {
            args.push(String::from("--help"));
        } else {
            args.extend(rest.split_whitespace().map(String::from));
            args.push(String::from("--help"));
        }
    } else {
        args.extend(input.split_whitespace().map(String::from));
    }

    let cli = match Cli::try_parse_from(args) {
        Ok(cli) => cli,
        Err(e) => {
            eprintln!("{e}");
            return;
        },
    };

    match cli.command {
        Command::Exit => exit::exit(),
        Command::Shell { command } => {
            let cmd = command.join(" ");
            shell::execute(&cmd, theme).await;
        },
        Command::Setup => {
            if let Err(e) = setup::execute(theme, &gctx.config_dir).await {
                eprintln!("Error: {}", e);
            }
        },
        Command::SaveHistory => {
            save_history::execute(session.history(), theme).await;
        },
        Command::ClearHistory => {
            session.clear_history();
            println!("{}", theme.green_text("Chat history cleared."));
        },
        Command::Compact => {
            compact::execute(session, theme).await;
        },
    }
}
