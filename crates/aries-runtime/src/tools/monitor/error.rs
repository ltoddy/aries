use crate::context::SpawnTaskError;

#[derive(thiserror::Error, Debug)]
pub enum MonitorError {
    #[error(transparent)]
    SpawnTask(#[from] SpawnTaskError),
}
