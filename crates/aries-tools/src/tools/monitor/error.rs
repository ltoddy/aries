#[derive(thiserror::Error, Debug)]
pub enum MonitorError {
    #[error("failed to start monitor command: {0}")]
    Io(#[from] std::io::Error),
}
