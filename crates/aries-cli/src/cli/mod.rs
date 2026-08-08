pub mod acp;
pub mod agent;
pub mod command;
pub mod exec;
pub mod mcp;
pub mod model;
pub mod prompt;
pub mod session;
pub mod setup;
pub mod skill;
pub mod stats;

use clap::{Parser, Subcommand};
use prompt::PromptArgs;

use crate::cli::acp::AcpArgs;
use crate::cli::agent::AgentCommand;
use crate::cli::command::CommandCommand;
use crate::cli::exec::ExecArgs;
use crate::cli::mcp::McpCommand;
use crate::cli::model::ModelCommand;
use crate::cli::session::SessionCommand;
use crate::cli::skill::SkillCommand;
use crate::cli::stats::StatsCommand;

#[derive(Parser, Debug, Clone)]
#[command(about = "Aries: your terminal AI assistant")]
pub struct Args {
    #[command(subcommand)]
    pub command: Option<Subcommands>,

    #[arg(long, help = "Run in bare mode")]
    pub bare: bool,
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
    #[command(about = "Manage slash commands")]
    Command {
        #[command(subcommand)]
        command: CommandCommand,
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
