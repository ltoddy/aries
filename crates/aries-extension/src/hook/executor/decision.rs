use std::collections::VecDeque;

#[derive(Debug, Clone)]
pub enum HookDecision {
    Continue { contexts: VecDeque<String> },
    Terminate { reason: String },
}

impl HookDecision {
    pub fn r#continue(contexts: impl IntoIterator<Item = String>) -> Self {
        Self::Continue { contexts: contexts.into_iter().collect() }
    }

    pub fn terminate(reason: impl Into<String>) -> Self {
        let reason = reason.into();
        Self::Terminate { reason }
    }
}
