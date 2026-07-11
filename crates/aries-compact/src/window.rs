//! 上下文窗口与 auto-compact 阈值计算。
//!
//! ```text
//! effective_context_window = context_window − reserved_output_tokens
//! auto_compact_threshold   = effective_context_window − AUTO_COMPACT_BUFFER_TOKENS
//! ```
//!
//! 当历史 + 即将发送的 prompt 估算 token 数 ≥ `auto_compact_threshold` 时触发压缩。

#[derive(Debug, Clone, Copy)]
pub struct ContextWindow {
    pub total: u64,
}

impl ContextWindow {
    const MAX_OUTPUT_TOKENS_FOR_SUMMARY: u64 = 20_000;

    const LARGE_CONTEXT_WINDOW: u64 = 1_000_000;
    const FALLBACK_CONTEXT_WINDOW: u64 = 128_000;

    const AUTO_COMPACT_BUFFER_TOKENS: u64 = 13_000;

    pub fn for_model(model: impl Into<String>) -> Self {
        let m = model.into().to_ascii_lowercase();
        let total = if m.contains("deepseek") || m.contains("gpt-5") {
            Self::LARGE_CONTEXT_WINDOW
        } else {
            Self::FALLBACK_CONTEXT_WINDOW
        };
        Self { total }
    }

    pub fn auto_compact_threshold(&self) -> u64 {
        self.effective().saturating_sub(Self::AUTO_COMPACT_BUFFER_TOKENS)
    }

    fn effective(&self) -> u64 {
        let reserved = Self::MAX_OUTPUT_TOKENS_FOR_SUMMARY;
        let effective = self.total.saturating_sub(reserved);
        // 下限保护：避免窗口配置过小使阈值变负 → 每条消息都触发 compact。
        effective.max(reserved + Self::AUTO_COMPACT_BUFFER_TOKENS)
    }
}
