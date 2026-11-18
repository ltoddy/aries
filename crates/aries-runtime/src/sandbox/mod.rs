#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;

use std::fmt::{Display, Formatter};

#[derive(Clone, Copy, Debug, Default, PartialOrd, PartialEq)]
pub enum SandboxMode {
    ReadOnly,
    #[default]
    WorkspaceWrite,
    Unrestricted,
}

impl Display for SandboxMode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            SandboxMode::ReadOnly => write!(f, "read-only"),
            SandboxMode::WorkspaceWrite => write!(f, "workspace-write"),
            SandboxMode::Unrestricted => write!(f, "unrestricted"),
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
