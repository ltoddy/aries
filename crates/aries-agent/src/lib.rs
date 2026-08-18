mod agent;
mod builder;
mod provider;

use rig::agent::StreamingError;
use rig::completion::{CompletionError, PromptError};

pub use self::agent::{AGENT_LOOP_MAX_TURNS, AriesAgent};
pub use self::builder::AgentBuilder;
pub use self::provider::AriesAgentProvider;

pub const AWAITING_USER_INPUT_REASON: &str = "awaiting user input";

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

    pub fn is_awaiting_user_input(&self) -> bool {
        matches!(
            self,
            AriesError::Streaming(StreamingError::Prompt(error)) if matches!(error.as_ref(), PromptError::PromptCancelled { reason, .. } if reason == AWAITING_USER_INPUT_REASON),
        )
    }
}

pub type AriesResult<T, E = AriesError> = Result<T, E>;
