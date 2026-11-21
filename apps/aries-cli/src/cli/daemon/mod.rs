#[cfg(any(target_os = "macos", target_os = "linux"))]
mod binary;
pub mod error;
pub mod install;
#[cfg(target_os = "macos")]
mod launchd;
pub mod status;
#[cfg(target_os = "linux")]
mod systemd;
use clap::Subcommand;

use self::install::InstallArgs;
use self::status::StatusArgs;

#[derive(Subcommand, Debug, Clone)]
pub enum DaemonCommand {
    #[command(about = "Install and start the daemon as a user service")]
    Install(InstallArgs),
    #[command(about = "Show the daemon's user service status")]
    Status(StatusArgs),
}
