use std::path::PathBuf;
use std::process::Stdio;

use aries_tools::shell::detect_shell;
use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[command(about = "Execute a shell command")]
pub struct ExecArgs {
    #[arg(
        trailing_var_arg = true,
        allow_hyphen_values = true,
        help = "The command and its arguments"
    )]
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

    let shell = detect_shell();
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let mut cmd = shell.build_command(&args.command.join(" "), &cwd);
    let status = cmd
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .await?;

    if !status.success() {
        std::process::exit(status.code().unwrap_or(-1));
    }

    Ok(())
}
