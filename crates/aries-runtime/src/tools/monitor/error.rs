use crate::tools::task::SpawnTaskError;

#[derive(thiserror::Error, Debug)]
pub enum MonitorError {
    #[error(transparent)]
    SpawnTask(#[from] SpawnTaskError),
}

impl MonitorError {
    pub fn spawn_task(source: SpawnTaskError) -> Self {
        Self::SpawnTask(source)
    }
}
