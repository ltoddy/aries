#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "macos")]
use std::ffi::OsString;
use std::fmt::{Display, Formatter};
use std::path::Path;
use std::str::FromStr;

use tokio::process::Command;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SandboxMode {
    ReadOnly,
    #[default]
    WorkspaceWrite,
    Unrestricted,
}

impl Display for SandboxMode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid sandbox mode `{0}`: expected `read-only`, `workspace-write`, or `unrestricted`")]
pub struct ParseSandboxModeError(String);

impl ParseSandboxModeError {
    pub fn new(mode: impl Into<String>) -> Self {
        Self(mode.into())
    }
}

impl FromStr for SandboxMode {
    type Err = ParseSandboxModeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "read-only" => Ok(SandboxMode::ReadOnly),
            "workspace-write" => Ok(SandboxMode::WorkspaceWrite),
            "unrestricted" => Ok(SandboxMode::Unrestricted),
            _ => Err(ParseSandboxModeError::new(s)),
        }
    }
}

impl SandboxMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            SandboxMode::ReadOnly => "read-only",
            SandboxMode::WorkspaceWrite => "workspace-write",
            SandboxMode::Unrestricted => "unrestricted",
        }
    }

    pub async fn command(
        &self,
        shell: impl AsRef<str>,
        home: impl AsRef<Path>,
        cwd: impl AsRef<Path>,
    ) -> std::io::Result<Command> {
        let shell = shell.as_ref();
        let home = home.as_ref();
        let cwd = cwd.as_ref();

        #[cfg(target_os = "macos")]
        {
            let home = tokio::fs::canonicalize(home).await?;
            let cwd = tokio::fs::canonicalize(cwd).await?;
            let temp_dir = tokio::fs::canonicalize(std::env::temp_dir()).await?;
            let mut command = Command::new("/usr/bin/sandbox-exec");
            command.arg("-p").arg(self.seatbelt_profile());
            for (name, path) in [
                ("HOME", home.as_path()),
                ("WORKSPACE", cwd.as_path()),
                ("SANDBOX_TMP", temp_dir.as_path()),
            ] {
                let mut parameter = OsString::from(name);
                parameter.push("=");
                parameter.push(path);
                command.arg("-D").arg(parameter);
            }
            command.env("TMPDIR", &temp_dir).arg(shell);
            Ok(command)
        }

        #[cfg(not(target_os = "macos"))]
        {
            let _ = (home, cwd);
            Ok(Command::new(shell))
        }
    }
}

#[cfg(target_os = "macos")]
impl SandboxMode {
    pub fn seatbelt_profile(&self) -> &'static str {
        match self {
            SandboxMode::ReadOnly => macos::READ_ONLY_PROFILE,
            SandboxMode::WorkspaceWrite => macos::WORKSPACE_WRITE_PROFILE,
            SandboxMode::Unrestricted => macos::UNRESTRICTED_PROFILE,
        }
    }
}
