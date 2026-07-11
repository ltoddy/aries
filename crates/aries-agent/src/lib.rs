pub mod agent;
pub mod builder;
pub mod compact_agent;
pub mod event;
pub mod mode;
pub mod preamble;
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

// Re-exports to match the old `aries_core` paths
pub use crate::agent::{AGENT_LOOP_MAX_TURNS, AriesAgent};
pub use crate::builder::AgentBuilder;
pub use crate::compact_agent::{CompactAgent, CompactOutcome, compact_summary};
pub use crate::event::{AgentEvent, AgentSignal, earse};
pub use crate::mode::Mode;
