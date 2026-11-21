#[cfg(any(target_os = "macos", target_os = "linux"))]
use std::io;
#[cfg(target_os = "macos")]
use std::num::ParseIntError;
#[cfg(any(target_os = "macos", target_os = "linux"))]
use std::path::{Path, PathBuf};
#[cfg(any(target_os = "macos", target_os = "linux"))]
use std::process::{ExitStatus, Output};

#[derive(Debug, thiserror::Error)]
pub enum DaemonError {
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    #[error("daemon management is only supported on macOS and Linux")]
    Unsupported,

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[error("{operation}: {source}")]
    Io { operation: String, source: io::Error },

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[error("invalid daemon binary {}: {reason}", path.display())]
    InvalidBinary { path: PathBuf, reason: &'static str },

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[error("invalid path for daemon service configuration {}: {reason}", path.display())]
    InvalidPath { path: PathBuf, reason: &'static str },

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[error("daemon service configuration already exists at {}; refusing to overwrite it or reinstall the service", path.display())]
    ExistingConfig { path: PathBuf },

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[error(
        "daemon service {service} is already registered; refusing to reinstall or disturb the existing service"
    )]
    AlreadyRegistered { service: String },

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[error("{command} failed ({status}); stdout: {stdout}; stderr: {stderr}")]
    Command { command: String, status: ExitStatus, stdout: String, stderr: String },

    #[cfg(target_os = "macos")]
    #[error("invalid UID returned by /usr/bin/id -u: {output:?}: {source}")]
    InvalidUid { output: String, source: ParseIntError },

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[error("service manager returned an unexpected service status: {output}")]
    InvalidStatus { output: String },
}

impl DaemonError {
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    pub fn unsupported() -> Self {
        Self::Unsupported
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    pub fn io(operation: String, source: io::Error) -> Self {
        Self::Io { operation, source }
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    pub fn invalid_binary(path: &Path, reason: &'static str) -> Self {
        Self::InvalidBinary { path: path.to_path_buf(), reason }
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    pub fn invalid_path(path: &Path, reason: &'static str) -> Self {
        Self::InvalidPath { path: path.to_path_buf(), reason }
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    pub fn existing_config(path: &Path) -> Self {
        Self::ExistingConfig { path: path.to_path_buf() }
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    pub fn already_registered(service: String) -> Self {
        Self::AlreadyRegistered { service }
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    pub fn command(command: String, output: Output) -> Self {
        Self::Command {
            command,
            status: output.status,
            stdout: String::from_utf8_lossy(&output.stdout).trim().to_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        }
    }

    #[cfg(target_os = "macos")]
    pub fn invalid_uid(output: String, source: ParseIntError) -> Self {
        Self::InvalidUid { output, source }
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    pub fn invalid_status(output: String) -> Self {
        Self::InvalidStatus { output }
    }
}
