use std::fs::OpenOptions;
use std::io::{self, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

use aries_init::GlobalContext;
use tokio::process::Command;

use crate::Result;
use crate::cli::daemon::binary;
use crate::cli::daemon::error::DaemonError;
use crate::cli::daemon::launchd::{self, LABEL};

const THROTTLE_INTERVAL: i64 = 10;
const UMASK: i64 = 0o077;

pub async fn execute(gctx: GlobalContext) -> Result<(), DaemonError> {
    let launch_agents = gctx.home_dir().join("Library/LaunchAgents");
    let plist_path = launch_agents.join(format!("{LABEL}.plist"));
    ensure_config_absent(&plist_path).await?;

    let binary = binary::resolve().await?;
    let domain = launchd::domain().await?;
    if launchd::service(&domain).await?.is_some() {
        return Err(DaemonError::already_registered(format!("{domain}/{LABEL}")));
    }

    let logs = gctx.root_dir().join("logs");
    let xml = launch_agent(&binary, &logs)?;

    tokio::fs::create_dir_all(&logs).await.map_err(|source| {
        DaemonError::io(format!("failed to create daemon log directory {}", logs.display()), source)
    })?;
    tokio::fs::create_dir_all(&launch_agents).await.map_err(|source| {
        DaemonError::io(
            format!("failed to create LaunchAgents directory {}", launch_agents.display()),
            source,
        )
    })?;
    write_plist(&plist_path, xml.as_bytes())?;

    let command = format!(
        "/bin/launchctl bootstrap {domain} {}; new plist retained on failure, inspect the registration before removing it or retrying",
        plist_path.display()
    );
    let output = Command::new("/bin/launchctl")
        .arg("bootstrap")
        .arg(&domain)
        .arg(&plist_path)
        .output()
        .await
        .map_err(|source| DaemonError::io(format!("failed to execute {command}"), source))?;
    if !output.status.success() {
        return Err(DaemonError::command(command, output));
    }

    println!("Installed {LABEL} in {domain}");
    println!("LaunchAgent: {}", plist_path.display());
    println!("Daemon binary: {}", binary.display());
    println!("Logs: {}", logs.display());
    Ok(())
}

async fn ensure_config_absent(path: &Path) -> Result<(), DaemonError> {
    match tokio::fs::symlink_metadata(path).await {
        Ok(_) => Err(DaemonError::existing_config(path)),
        Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(DaemonError::io(
            format!("failed to check LaunchAgent plist {}", path.display()),
            source,
        )),
    }
}

fn launch_agent(binary: &Path, logs: &Path) -> Result<String, DaemonError> {
    let binary = xml_path(binary)?;
    let stdout = xml_path(&logs.join("daemon.stdout.log"))?;
    let stderr = xml_path(&logs.join("daemon.stderr.log"))?;
    Ok(format!(
        include_str!("macos.plist"),
        LABEL = LABEL,
        binary = binary,
        THROTTLE_INTERVAL = THROTTLE_INTERVAL,
        UMASK = UMASK,
        stdout = stdout,
        stderr = stderr,
    ))
}

fn xml_path(path: &Path) -> Result<String, DaemonError> {
    let value =
        path.to_str().ok_or_else(|| DaemonError::invalid_path(path, "path is not UTF-8"))?;
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            '\r' => escaped.push_str("&#13;"),
            '\t'
            | '\n'
            | '\u{20}'..='\u{d7ff}'
            | '\u{e000}'..='\u{fffd}'
            | '\u{10000}'..='\u{10ffff}' => {
                escaped.push(character);
            },
            _ => {
                return Err(DaemonError::invalid_path(
                    path,
                    "path contains an invalid XML 1.0 character",
                ));
            },
        }
    }
    Ok(escaped)
}

fn write_plist(path: &Path, xml: &[u8]) -> Result<(), DaemonError> {
    let mut file = OpenOptions::new().write(true).create_new(true).mode(0o600).open(path).map_err(
        |source| {
            if source.kind() == io::ErrorKind::AlreadyExists {
                DaemonError::existing_config(path)
            } else {
                DaemonError::io(
                    format!("failed to create LaunchAgent plist {}", path.display()),
                    source,
                )
            }
        },
    )?;
    file.write_all(xml).map_err(|source| {
        DaemonError::io(
            format!("failed to write new LaunchAgent plist {}; inspect the retained file before retrying", path.display()),
            source,
        )
    })?;
    file.sync_all().map_err(|source| {
        DaemonError::io(
            format!("failed to sync new LaunchAgent plist {}; inspect the retained file before retrying", path.display()),
            source,
        )
    })
}
