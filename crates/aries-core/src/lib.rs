pub mod agents;
pub mod compact;
pub mod event;
pub mod ext;
pub mod fs;
pub mod jsonrpc;
pub mod language_server;
pub mod preamble;
pub mod repository;
pub mod tools;

use rig_core::agent::StreamingError;
use rig_core::completion::CompletionError;

#[derive(Debug, thiserror::Error)]
pub enum AriesError {
    #[error("{0}")]
    Streaming(#[from] StreamingError),
}

impl AriesError {
    pub fn is_context_exceeded(&self) -> bool {
        const PATTERNS: [&str; 6] = [
            "prompt_too_long",
            "context_length_exceeded",
            "maximum context length",
            "context length exceeded",
            "too many tokens",
            "input is too long",
        ];

        if let AriesError::Streaming(StreamingError::Completion(CompletionError::ProviderError(
            err,
        ))) = self
        {
            return PATTERNS.iter().any(|p| err.contains(p));
        }
        false
    }
}

pub type AriesResult<T, E = AriesError> = Result<T, E>;
