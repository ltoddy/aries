use std::process::Stdio;

use clap::Parser;
use tokio::process::Command;

#[derive(Parser, Debug, Clone)]
pub struct ExecArgs {
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub command: Vec<String>,
}

pub async fn execute(args: ExecArgs) -> anyhow::Result<()> {
    if args.command.is_empty() {
        return Ok(());
    }

    if let Some(output) = aries_tokenopt::execute(&args.command).await {
        print!("{}", output.stdout);
        eprint!("{}", output.stderr);
        std::process::exit(output.exit_code);
    }

    let shell = std::env::var("SHELL").unwrap_or(String::from("bash"));
    let status = Command::new(shell)
        .arg("-c")
        .arg(args.command.join(" "))
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .await?;

    if !status.success() {
        std::process::exit(status.code().unwrap_or(-1));
    }

    Ok(())
}
