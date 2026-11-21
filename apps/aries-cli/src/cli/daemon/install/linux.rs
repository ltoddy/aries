use std::fs::OpenOptions;
use std::io::{self, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

use aries_init::GlobalContext;

use crate::cli::daemon::binary;
use crate::cli::daemon::error::DaemonError;
use crate::cli::daemon::systemd::{
    UNIT, ensure_service_absent, systemctl, unit_file_exists, unit_path, validate_path,
};

pub async fn execute(gctx: GlobalContext) -> crate::Result<(), DaemonError> {
    let path = unit_path(gctx.home_dir())?;
    if unit_file_exists(&path).await? {
        return Err(DaemonError::existing_config(&path));
    }
    ensure_service_absent().await?;

    let binary = binary::resolve().await?;
    let unit = service_unit(&binary)?;
    let directory = path
        .parent()
        .ok_or_else(|| DaemonError::invalid_path(&path, "the unit file has no parent directory"))?;
    tokio::fs::create_dir_all(directory).await.map_err(|source| {
        DaemonError::io(
            format!("failed to create systemd user unit directory {}", directory.display()),
            source,
        )
    })?;
    write_unit(&path, unit.as_bytes())?;
    install_command(&["daemon-reload"], &path).await?;
    install_command(&["enable", "--now", validate_path(&path)?], &path).await?;

    println!("Installed {UNIT} (systemd --user)");
    println!("Unit file: {}", path.display());
    println!("Daemon binary: {}", binary.display());
    println!("Logs: journalctl --user -u {UNIT} -f");
    println!(
        "To start at boot and keep running after logout, run loginctl enable-linger \"$USER\" separately"
    );
    Ok(())
}

fn escape_path(path: &Path) -> crate::Result<String, DaemonError> {
    let value = validate_path(path)?;
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '%' => escaped.push_str("%%"),
            _ => escaped.push(character),
        }
    }
    Ok(escaped)
}

fn service_unit(binary: &Path) -> crate::Result<String, DaemonError> {
    if validate_path(binary)?.contains(['\'', '"', '\\']) {
        return Err(DaemonError::invalid_binary(
            binary,
            "systemd executable paths cannot contain single quotes, double quotes, or backslashes",
        ));
    }
    let binary = escape_path(binary)?;
    Ok(format!(include_str!("linux.service"), binary = binary))
}

fn write_unit(path: &Path, unit: &[u8]) -> crate::Result<(), DaemonError> {
    let mut file = OpenOptions::new().write(true).create_new(true).mode(0o600).open(path).map_err(
        |source| {
            if source.kind() == io::ErrorKind::AlreadyExists {
                DaemonError::existing_config(path)
            } else {
                DaemonError::io(
                    format!("failed to create systemd user unit file {}", path.display()),
                    source,
                )
            }
        },
    )?;
    file.write_all(unit).map_err(|source| {
        DaemonError::io(
            format!(
                "failed to write {}; partial installation retained, no rollback performed; inspect the unit file before retrying",
                path.display()
            ),
            source,
        )
    })?;
    file.sync_all().map_err(|source| {
        DaemonError::io(
            format!(
                "failed to sync {}; partial installation retained, no rollback performed; inspect the unit file before retrying",
                path.display()
            ),
            source,
        )
    })
}

async fn install_command(args: &[&str], path: &Path) -> crate::Result<(), DaemonError> {
    let context = format!(
        "; partial installation retained at {}, enablement or runtime state may have changed; no rollback performed; inspect systemctl --user status {UNIT} and journalctl --user -u {UNIT} before retrying",
        path.display()
    );
    let output = systemctl(args, &context).await?;
    if !output.status.success() {
        return Err(DaemonError::command(
            format!("systemctl --user {}{context}", args.join(" ")),
            output,
        ));
    }
    Ok(())
}
