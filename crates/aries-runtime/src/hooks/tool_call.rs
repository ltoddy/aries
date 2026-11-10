use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use aries_persistence::ToolCallRepository;
use rig::agent::{
    AgentHook, DispatchAction, DispatchEvent, HookContext, OutcomeAction, OutcomeEvent,
};
use rig::effect::{Answered, EffectKind};
use rig::message::CallId;
use toasty::Db;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct ToolCallHook {
    cwd: PathBuf,
    tool_call_repo: ToolCallRepository,
    tool_call_started_at: Arc<Mutex<HashMap<CallId, Instant>>>,
}

impl ToolCallHook {
    pub fn new(cwd: impl AsRef<Path>, db: Db) -> Self {
        let cwd = cwd.as_ref().to_owned();
        let tool_call_repo = ToolCallRepository::new(db);

        Self { cwd, tool_call_repo, tool_call_started_at: Arc::new(Mutex::new(HashMap::new())) }
    }
}

impl AgentHook for ToolCallHook {
    async fn on_dispatch(&self, _ctx: &HookContext, event: DispatchEvent<'_>) -> DispatchAction {
        if matches!(event.kind, EffectKind::ToolCall { .. })
            && let Some(call_id) = event.call_id
        {
            self.tool_call_started_at.lock().await.insert(call_id.clone(), Instant::now());
        }

        DispatchAction::proceed()
    }

    async fn on_outcome(&self, _ctx: &HookContext, event: OutcomeEvent<'_>) -> OutcomeAction {
        let EffectKind::ToolCall { name, args } = event.kind else {
            return OutcomeAction::proceed();
        };
        let Some(call_id) = event.call_id else {
            return OutcomeAction::proceed();
        };
        let was_successful = match event.outcome {
            Ok(Answered::ToolResult { result }) => result.is_success(),
            Err(_) => false,
            _ => return OutcomeAction::proceed(),
        };

        let duration_ms = self
            .tool_call_started_at
            .lock()
            .await
            .remove(call_id)
            .map(|started_at| started_at.elapsed().as_millis() as u64);

        let cwd = self.cwd.display().to_string();
        let call_id = call_id.to_string();
        let name = name.clone();
        let args = args.clone();
        let mut repo = self.tool_call_repo.clone();

        tokio::task::spawn(async move {
            let _ = repo.create(cwd, call_id, name, args, duration_ms, was_successful).await;
        });

        OutcomeAction::proceed()
    }
}
