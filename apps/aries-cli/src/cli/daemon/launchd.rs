use tokio::process::Command;

use crate::Result;
use crate::cli::daemon::error::DaemonError;

pub const LABEL: &str = "com.aries.daemon";
const SERVICE_NOT_FOUND: i32 = 113;

pub async fn domain() -> Result<String, DaemonError> {
    let command = "/usr/bin/id -u";
    let output = Command::new("/usr/bin/id")
        .arg("-u")
        .output()
        .await
        .map_err(|source| DaemonError::io(format!("failed to execute {command}"), source))?;
    if !output.status.success() {
        return Err(DaemonError::command(command.to_owned(), output));
    }
    let output = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    let uid = output.parse::<u32>().map_err(|source| DaemonError::invalid_uid(output, source))?;
    Ok(format!("gui/{uid}"))
}

pub async fn service(domain: &str) -> Result<Option<String>, DaemonError> {
    let command = format!("/bin/launchctl print {domain}");
    let output = Command::new("/bin/launchctl")
        .arg("print")
        .arg(domain)
        .output()
        .await
        .map_err(|source| DaemonError::io(format!("failed to execute {command}"), source))?;
    if !output.status.success() {
        return Err(DaemonError::command(command, output));
    }

    let service = format!("{domain}/{LABEL}");
    let command = format!("/bin/launchctl print {service}");
    let output = Command::new("/bin/launchctl")
        .arg("print")
        .arg(&service)
        .output()
        .await
        .map_err(|source| DaemonError::io(format!("failed to execute {command}"), source))?;
    if output.status.success() {
        return Ok(Some(String::from_utf8_lossy(&output.stdout).into_owned()));
    }
    if output.status.code() == Some(SERVICE_NOT_FOUND) {
        return Ok(None);
    }
    Err(DaemonError::command(command, output))
}
