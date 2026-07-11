#[derive(thiserror::Error, Debug)]
pub enum AskUserQuestionError {
    #[error("Failed to ask question: {0}")]
    InteractionError(String),
}
