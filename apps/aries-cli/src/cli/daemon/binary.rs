use std::env;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use crate::Result;
use crate::cli::daemon::error::DaemonError;

pub async fn resolve() -> Result<PathBuf, DaemonError> {
    let executable = env::current_exe().map_err(|source| {
        DaemonError::io("failed to locate the current aries executable".to_owned(), source)
    })?;
    let directory = executable.parent().ok_or_else(|| {
        DaemonError::invalid_binary(&executable, "the aries executable has no parent directory")
    })?;
    let path = directory.join("aries-daemon");
    let binary = tokio::fs::canonicalize(&path).await.map_err(|source| {
        DaemonError::io(
            format!(
                "failed to resolve daemon binary {}; install aries-daemon separately beside aries",
                path.display()
            ),
            source,
        )
    })?;
    let metadata = tokio::fs::metadata(&binary).await.map_err(|source| {
        DaemonError::io(format!("failed to inspect daemon binary {}", binary.display()), source)
    })?;
    if !metadata.is_file() {
        return Err(DaemonError::invalid_binary(&binary, "not a regular file"));
    }
    if metadata.permissions().mode() & 0o111 == 0 {
        return Err(DaemonError::invalid_binary(&binary, "no executable permission bits are set"));
    }
    Ok(binary)
}
