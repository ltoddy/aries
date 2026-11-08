use std::path::PathBuf;

use crate::context::SpawnTaskError;

#[derive(thiserror::Error, Debug)]
pub enum BashError {
    #[error(
        "failed to execute command using shell '{shell}' in {cwd} for command {command:?}: {source}"
    )]
    Io { shell: String, cwd: PathBuf, command: String, source: std::io::Error },

    #[error(transparent)]
    SpawnTask(#[from] SpawnTaskError),
}

impl BashError {
    pub fn io(
        shell: impl Into<String>,
        cwd: impl Into<PathBuf>,
        command: impl Into<String>,
        source: std::io::Error,
    ) -> BashError {
        let shell = shell.into();
        let cwd = cwd.into();
        let command = command.into();

        Self::Io { shell, cwd, command, source }
    }
}
