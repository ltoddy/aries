#[derive(thiserror::Error, Debug)]
pub enum BashError {
    #[error("failed to execute command: {0}")]
    Io(#[from] std::io::Error),
}

impl BashError {
    pub fn io(err: std::io::Error) -> BashError {
        Self::Io(err)
    }
}
