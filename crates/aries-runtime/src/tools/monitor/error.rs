use crate::tools::task::SpawnTaskError;

#[derive(thiserror::Error, Debug)]
pub enum MonitorError {
    #[error(transparent)]
    SpawnTask(#[from] SpawnTaskError),
}
