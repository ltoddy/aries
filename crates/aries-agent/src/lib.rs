mod agent;
mod builder;
mod provider;

use rig_agent::agent::StreamingError;
use rig_agent::completion::CompletionError;

pub use self::agent::{AGENT_LOOP_MAX_TURNS, AriesAgent};
pub use self::builder::AgentBuilder;
pub use self::provider::AriesAgentProvider;

#[derive(Debug, thiserror::Error)]
pub enum AriesError {
    #[error("{0}")]
    Streaming(#[from] StreamingError),

    #[error("hook terminated: {0}")]
    HookTerminated(String),
}

impl AriesError {
    pub fn hook_terminated(reason: impl Into<String>) -> Self {
        let reason = reason.into();
        Self::HookTerminated(reason)
    }

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
