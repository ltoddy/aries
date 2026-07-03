mod diff;
mod log;
mod status;

use std::ffi::OsString;

use clap::{Parser, Subcommand};
use thiserror::Error;

use crate::Output;

#[derive(Debug, Error)]
pub enum GitError {
    #[error("git command failed: {0}")]
    Exec(#[from] std::io::Error),
    #[error("not a git repository (or any of the parent directories)")]
    NotARepo,
}

pub async fn execute<I, S>(args: I) -> Option<Output>
where
    I: IntoIterator<Item = S>,
    S: Into<OsString> + Clone,
{
    let root = GitRoot::try_parse_from(args).ok()?;
    let rest_args = root.rest_args();
    let command = root.command?;

    match command {
        GitSubcommand::Status { args } => status::execute(args, rest_args).ok(),
    }
}

#[derive(Debug, Parser)]
#[command(no_binary_name = true)]
pub struct GitRoot {
    #[arg(short = 'C', value_name = "path", num_args = 1)]
    capital_c: Option<String>,

    #[arg(long = "work-tree", value_name = "path", num_args = 1)]
    work_tree: Option<String>,

    #[command(subcommand)]
    command: Option<GitSubcommand>,
}

impl GitRoot {
    pub fn rest_args(&self) -> Vec<String> {
        let mut args = Vec::new();
        if let Some(ref val) = self.capital_c {
            args.push("-C".to_owned());
            args.push(val.clone());
        }
        if let Some(ref val) = self.work_tree {
            args.push("--work-tree".to_owned());
            args.push(val.clone());
        }
        args
    }
}

#[derive(Debug, Subcommand)]
pub enum GitSubcommand {
    #[command(visible_alias = "st")]
    Status {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
}
