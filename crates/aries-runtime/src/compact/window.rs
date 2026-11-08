#[derive(Debug, Clone, Copy)]
pub struct ContextWindow {
    pub total: u64,
}

// rig 没有提供方法来获取模型对应的上下文窗口大小
// 目前是 2026 年了, 模型拥有 1M 上下文窗口或者大于这个窗口是很常见的了,所以这里目前先 hardcode 1M.

const CONTEXT_WINDOW: u64 = 1_000_000;

impl Default for ContextWindow {
    fn default() -> Self {
        Self::new()
    }
}

impl ContextWindow {
    pub fn new() -> Self {
        Self { total: CONTEXT_WINDOW }
    }

    pub fn auto_compact_threshold(&self) -> u64 {
        self.sixty_percent_threshold()
    }

    pub fn sixty_percent_threshold(&self) -> u64 {
        self.total / 10 * 6
    }

    pub fn sixty_five_percent_threshold(&self) -> u64 {
        self.total / 100 * 65
    }

    pub fn seventy_percent_threshold(&self) -> u64 {
        self.total / 10 * 7
    }

    pub fn seventy_five_percent_threshold(&self) -> u64 {
        self.total / 100 * 75
    }

    pub fn eighty_percent_threshold(&self) -> u64 {
        self.total / 10 * 8
    }
}
