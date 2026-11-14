use std::io;
use std::sync::Arc;
use std::time::Duration;

use rig::completion::PromptError;
use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum CompactionError {
    #[error(
        "压缩处于冷却期（已连续失败 {consecutive_failures} 次），约 {wait:?} 后重试，本次跳过。"
    )]
    Cooldown { wait: Duration, consecutive_failures: usize },

    #[error("上下文过长，压缩请求被拒，进入冷却以避免反复重试。")]
    PromptTooLong,

    #[error("压缩遇到临时错误（不计入失败）：{0}")]
    Transient(#[source] Arc<CompactionFailure>),

    #[error("压缩结果为空。")]
    Empty,

    #[error("连续 {failures} 次压缩失败，进入 {cooldown_minutes} 分钟冷却。")]
    TooManyFailures { failures: usize, cooldown_minutes: u64 },
}

impl CompactionError {
    pub fn cooldown(wait: Duration, consecutive_failures: usize) -> Self {
        Self::Cooldown { wait, consecutive_failures }
    }

    pub fn prompt_too_long() -> Self {
        Self::PromptTooLong
    }

    pub fn transient(error: CompactionFailure) -> Self {
        Self::Transient(Arc::new(error))
    }

    pub fn empty() -> Self {
        Self::Empty
    }

    pub fn too_many_failures(failures: usize, cooldown_minutes: u64) -> Self {
        Self::TooManyFailures { failures, cooldown_minutes }
    }
}

#[derive(Debug, Error)]
pub enum CompactionFailure {
    #[error("save transcript failed: {0}")]
    SaveTranscript(#[source] io::Error),

    #[error("{0}")]
    Prompt(#[source] Box<PromptError>),
}

impl CompactionFailure {
    pub fn save_transcript(error: io::Error) -> Self {
        Self::SaveTranscript(error)
    }

    pub fn prompt(error: PromptError) -> Self {
        Self::Prompt(Box::new(error))
    }
}
