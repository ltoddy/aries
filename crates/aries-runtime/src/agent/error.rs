use rig::ProviderError;
use rig::completion::PromptError;

#[derive(Debug, thiserror::Error)]
pub enum AriesError {
    #[error("{0}")]
    Streaming(#[source] Box<PromptError>),

    #[error("hook terminated: {0}")]
    HookTerminated(String),
}

impl AriesError {
    pub fn streaming(err: PromptError) -> Self {
        Self::Streaming(Box::new(err))
    }

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
            && let PromptError::Provider(
                ProviderError::Provider(message) | ProviderError::Response(message),
            ) = err.as_ref()
        {
            return PATTERNS.iter().any(|p| message.contains(p));
        }
        false
    }
}
