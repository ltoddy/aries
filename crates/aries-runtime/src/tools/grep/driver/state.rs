use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

pub struct ResultLimit {
    limit: usize,
    emitted: AtomicUsize,
    truncated: AtomicBool,
}

impl ResultLimit {
    pub fn new(limit: usize) -> Self {
        Self { limit, emitted: AtomicUsize::new(0), truncated: AtomicBool::new(false) }
    }

    pub fn try_acquire(&self) -> bool {
        if self.limit == 0 {
            return true;
        }
        let granted = self
            .emitted
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                (current < self.limit).then(|| current + 1)
            })
            .is_ok();
        if !granted {
            self.mark_truncated();
        }
        granted
    }

    pub fn limit(&self) -> usize {
        self.limit
    }

    pub fn mark_truncated(&self) {
        self.truncated.store(true, Ordering::Relaxed);
    }

    pub fn truncated(&self) -> bool {
        self.truncated.load(Ordering::Relaxed)
    }
}
