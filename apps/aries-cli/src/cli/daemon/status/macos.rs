use std::io;

use aries_init::GlobalContext;

use crate::Result;
use crate::cli::daemon::error::DaemonError;
use crate::cli::daemon::launchd::{self, LABEL};

pub async fn execute(gctx: GlobalContext) -> Result<(), DaemonError> {
    let plist_path = gctx.home_dir().join("Library/LaunchAgents").join(format!("{LABEL}.plist"));
    let installed = match tokio::fs::symlink_metadata(&plist_path).await {
        Ok(_) => true,
        Err(source) if source.kind() == io::ErrorKind::NotFound => false,
        Err(source) => {
            return Err(DaemonError::io(
                format!("failed to check LaunchAgent plist {}", plist_path.display()),
                source,
            ));
        },
    };
    let domain = launchd::domain().await?;
    match launchd::service(&domain).await? {
        Some(output) => {
            let state = field(&output, "state")
                .ok_or_else(|| DaemonError::invalid_status(output.clone()))?;
            println!("Status: {state}");
            println!("Service: {domain}/{LABEL}");
            if let Some(pid) = field(&output, "pid") {
                println!("PID: {pid}");
            }
            if let Some(exit_code) = field(&output, "last exit code") {
                println!("Last exit code: {exit_code}");
            }
            if let Some(signal) = field(&output, "last terminating signal") {
                println!("Last terminating signal: {signal}");
            }
        },
        None if installed => println!("Status: installed but not registered"),
        None => println!("Status: not installed"),
    }
    println!("LaunchAgent: {}", plist_path.display());
    println!("Logs: {}", gctx.root_dir().join("logs").display());
    Ok(())
}

fn field<'output>(output: &'output str, name: &str) -> Option<&'output str> {
    output.lines().find_map(|line| {
        let line = line.strip_prefix('\t')?;
        let (key, value) = line.split_once(" = ")?;
        (key == name).then(|| value.trim()).filter(|value| !value.is_empty())
    })
}
