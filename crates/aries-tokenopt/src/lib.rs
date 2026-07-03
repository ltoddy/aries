mod git;

use std::ffi::OsString;

use clap::{Parser, Subcommand};

#[derive(Debug, Clone, Default)]
pub struct Output {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

impl Output {
    pub fn new(exit_code: i32, stdout: impl Into<String>, stderr: impl Into<String>) -> Self {
        let stdout = stdout.into();
        let stderr = stderr.into();
        Self { exit_code, stdout, stderr }
    }

    pub fn stdout(stdout: impl Into<String>) -> Self {
        let stdout = stdout.into();
        Self { stdout, ..Default::default() }
    }
}

#[derive(Parser, Debug)]
#[command(no_binary_name = true, disable_help_flag = true, allow_external_subcommands = true)]
struct Root {
    #[command(subcommand)]
    command: Option<RootSubcommand>,
}

#[derive(Subcommand, Debug)]
enum RootSubcommand {
    Git {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
}

pub async fn execute<I, S>(args: I) -> Option<Output>
where
    I: IntoIterator<Item = S>,
    S: Into<OsString> + Clone,
{
    let command = Root::try_parse_from(args).ok().and_then(|args| args.command)?;

    match command {
        RootSubcommand::Git { args } => git::execute(args).await,
    }
}
