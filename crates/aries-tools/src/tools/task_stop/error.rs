use crate::context::StopTaskError;

#[derive(thiserror::Error, Debug)]
pub enum TaskStopError {
    #[error(transparent)]
    Stop(#[from] StopTaskError),
}

impl TaskStopError {
    pub fn stop(err: StopTaskError) -> Self {
        Self::Stop(err)
    }
}
