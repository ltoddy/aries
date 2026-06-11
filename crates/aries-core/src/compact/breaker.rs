use std::time::{Duration, Instant};

#[derive(Debug, Default, Clone)]
pub struct AutoCompactBreaker {
    consecutive_failures: usize,
    last_failure_at: Option<Instant>,
}

impl AutoCompactBreaker {
    pub const MAX_CONSECUTIVE_AUTOCOMPACT_FAILURES: usize = 3;
    pub const AUTOCOMPACT_FAILURE_COOLDOWN: Duration = Duration::from_secs(5 * 60);

    pub fn new() -> Self {
        Self::default()
    }

    pub fn consecutive_failures(&self) -> usize {
        self.consecutive_failures
    }

    pub fn decide(&self) -> Decision {
        if self.consecutive_failures < Self::MAX_CONSECUTIVE_AUTOCOMPACT_FAILURES {
            return Decision::Allow { half_open: false };
        }

        let now = Instant::now();
        let next_retry_at = self.last_failure_at.map(|at| at + Self::AUTOCOMPACT_FAILURE_COOLDOWN);
        match next_retry_at {
            Some(next_retry_at) if now < next_retry_at => Decision::Skip {
                wait: next_retry_at.saturating_duration_since(now),
                consecutive_failures: self.consecutive_failures,
            },
            _ => Decision::Allow { half_open: true },
        }
    }

    pub fn on_success(&mut self) {
        self.consecutive_failures = 0;
        self.last_failure_at = None;
    }

    pub fn on_failure(&mut self) {
        self.consecutive_failures = self.consecutive_failures.saturating_add(1);
        self.last_failure_at = Some(Instant::now());
    }

    /// 断路器领域的 标准动词 ——电气工程里 "the breaker tripped" 就是指开关跳闸进入断开状态。
    pub fn trip(&mut self) {
        self.consecutive_failures = Self::MAX_CONSECUTIVE_AUTOCOMPACT_FAILURES;
        self.last_failure_at = Some(Instant::now());
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Allow { half_open: bool },
    Skip { wait: Duration, consecutive_failures: usize },
}
