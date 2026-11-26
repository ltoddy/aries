use rig::catalog::Catalog;
use rig::providers::registry::{Format, ProviderId};

#[derive(Debug, Clone, Copy)]
pub struct ContextWindow {
    pub context_window: u64,
}

const KIB: u64 = 1024;
const DEFAULT_CONTEXT_WINDOW: u64 = 500 * KIB;

impl ContextWindow {
    pub fn new(vendor: impl AsRef<str>, model: impl AsRef<str>) -> Self {
        let vendor = vendor.as_ref();
        let model = model.as_ref();

        let mut context_window = DEFAULT_CONTEXT_WINDOW;
        if let Some(provider_id) = ProviderId::new(vendor, Format::OpenAi)
            && let Some(resolved) = Catalog::builtin().get(provider_id, model)
            && let Some(context_windows) = resolved.spec.context_window
        {
            context_window = context_windows as u64;
        }

        Self { context_window }
    }

    pub fn auto_compact_threshold(&self) -> u64 {
        self.sixty_percent_threshold()
    }

    pub fn sixty_percent_threshold(&self) -> u64 {
        self.context_window / 10 * 6
    }

    pub fn sixty_five_percent_threshold(&self) -> u64 {
        self.context_window / 100 * 65
    }

    pub fn seventy_percent_threshold(&self) -> u64 {
        self.context_window / 10 * 7
    }

    pub fn seventy_five_percent_threshold(&self) -> u64 {
        self.context_window / 100 * 75
    }

    pub fn eighty_percent_threshold(&self) -> u64 {
        self.context_window / 10 * 8
    }
}
