mod agent;
mod builder;

use rig::agent::StreamingError;

pub use self::agent::{AGENT_LOOP_MAX_TURNS, AriesAgent};
pub use self::builder::AgentBuilder;

#[derive(Debug, thiserror::Error)]
pub enum AriesError {
    #[error("{0}")]
    Streaming(#[source] Box<StreamingError>),

    #[error("hook terminated: {0}")]
    HookTerminated(String),
}

impl From<StreamingError> for AriesError {
    fn from(err: StreamingError) -> Self {
        Self::Streaming(Box::new(err))
    }
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

        if let AriesError::Streaming(err) = self
            && let StreamingError::Completion(err) = err.as_ref()
        {
            return PATTERNS.iter().any(|p| err.report().message.contains(p));
        }
        false
    }
}

pub type AriesResult<T, E = AriesError> = Result<T, E>;
