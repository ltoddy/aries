use std::path::PathBuf;

use aries_persistence::TokenUsageRepository;
use rig::agent::{AgentHook, HookContext, ModelTurnAction, ModelTurnFinished};
use toasty::Db;

#[derive(Clone)]
pub struct TokenUsageHook {
    cwd: PathBuf,
    token_usage_repo: TokenUsageRepository,
}

impl TokenUsageHook {
    pub fn new(cwd: impl Into<PathBuf>, db: Db) -> Self {
        let cwd = cwd.into();
        let token_usage_repo = TokenUsageRepository::new(db);

        Self { cwd, token_usage_repo }
    }
}

impl AgentHook for TokenUsageHook {
    async fn on_model_turn_finished(
        &self,
        _ctx: &HookContext,
        event: ModelTurnFinished<'_>,
    ) -> ModelTurnAction {
        let cwd = self.cwd.display().to_string();
        let mut repo = self.token_usage_repo.clone();

        let input_tokens = event.usage.input_tokens.unwrap_or(0);
        let output_tokens = event.usage.output_tokens.unwrap_or(0);
        let total_tokens = event.usage.total_tokens.unwrap_or(0);
        let cached_input_tokens = event.usage.cached_input_tokens.unwrap_or(0);
        let reasoning_tokens = event.usage.reasoning_tokens.unwrap_or(0);

        tokio::task::spawn(async move {
            let _ = repo
                .create(
                    cwd,
                    input_tokens,
                    output_tokens,
                    total_tokens,
                    cached_input_tokens,
                    reasoning_tokens,
                )
                .await;
        });

        ModelTurnAction::Continue
    }
}
