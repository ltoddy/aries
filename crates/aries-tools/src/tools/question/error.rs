#[derive(thiserror::Error, Debug)]
pub enum AskUserQuestionError {
    #[error("failed to ask question: {0}")]
    InteractionError(String),
}
