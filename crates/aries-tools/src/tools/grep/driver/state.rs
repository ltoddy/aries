use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

#[derive(Default)]
pub struct StopState {
    limit: usize,
    emitted: AtomicUsize,
    truncated: AtomicBool,
}

impl StopState {
    pub fn new(limit: usize) -> Self {
        let emitted = AtomicUsize::new(0);
        let truncated = AtomicBool::new(false);

        Self { limit, emitted, truncated }
    }

    pub fn grant(&self, amount: usize) -> usize {
        if self.limit == 0 {
            self.emitted.fetch_add(amount, Ordering::Relaxed);
            return amount;
        }

        loop {
            let current = self.emitted.load(Ordering::Relaxed);
            if current >= self.limit {
                self.truncated.store(true, Ordering::Relaxed);
                return 0;
            }
            let remaining = self.limit - current;
            let granted = remaining.min(amount);
            let next = current + granted;

            if self
                .emitted
                .compare_exchange(current, next, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
            {
                if granted < amount {
                    self.truncated.store(true, Ordering::Relaxed);
                }
                return granted;
            }
        }
    }

    pub fn should_stop(&self) -> bool {
        if self.limit == 0 {
            return false;
        }

        if self.emitted.load(Ordering::Relaxed) >= self.limit {
            self.truncated.store(true, Ordering::Relaxed);
            return true;
        }
        false
    }

    pub fn limit(&self) -> usize {
        self.limit
    }

    pub fn truncated(&self) -> bool {
        self.truncated.load(Ordering::Relaxed)
    }
}
