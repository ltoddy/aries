#[derive(Debug, Clone, Copy)]
pub struct ContextWindow {
    pub total: u64,
}

const KIB: u64 = 1024;
const CONTEXT_WINDOW: u64 = 500 * KIB;

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
