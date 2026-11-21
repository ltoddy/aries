use aries_init::GlobalContext;
use clap::Parser;

use crate::Result;
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
use crate::cli::daemon::error::DaemonError;
use crate::error::Error;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;

#[derive(Clone, Debug, Parser)]
pub struct InstallArgs {}

#[cfg(target_os = "macos")]
pub async fn execute(_args: InstallArgs, gctx: GlobalContext) -> Result<()> {
    macos::execute(gctx).await.map_err(Error::daemon)
}

#[cfg(target_os = "linux")]
pub async fn execute(_args: InstallArgs, gctx: GlobalContext) -> Result<()> {
    linux::execute(gctx).await.map_err(Error::daemon)
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub async fn execute(_args: InstallArgs, _: GlobalContext) -> Result<()> {
    Err(Error::daemon(DaemonError::unsupported()))
}
