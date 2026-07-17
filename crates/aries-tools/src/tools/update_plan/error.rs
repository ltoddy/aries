#[derive(thiserror::Error, Debug)]
pub enum UpdatePlanError {
    #[error("Failed to send plan update: {0}")]
    SendFailed(String),
}
