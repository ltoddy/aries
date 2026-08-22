use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use aries_preamble::agentsmd::Agentsmd;
use tokio::sync::Mutex;

#[derive(Clone, Debug, Default)]
pub struct InstructionContext(Arc<Mutex<InstructionState>>);

impl InstructionContext {
    pub fn new(root_dir: impl AsRef<Path>) -> Self {
        Self(Arc::new(Mutex::new(InstructionState::new(root_dir))))
    }

    pub async fn visit(&self, dir: impl AsRef<Path>) {
        let dir = dir.as_ref();

        let mut guard = self.0.lock().await;

        if guard.root_dir == dir {
            // 根目录的 AGENTS.md 会作为 system prompt 的一部分给到大模型
            return;
        }

        let file_path = dir.join(aries_preamble::agentsmd::FILENAME);
        // 已经访问过了, 什么也不做
        if guard.instructions.iter().find(|i| i.file_path == file_path).is_some() {
            return;
        }

        let Ok(content) = tokio::fs::read_to_string(&file_path).await else { return };

        let instruction = Agentsmd::new(file_path, content, false);
        guard.instructions.push(instruction.clone());
        guard.pending_instructions.push_back(instruction);
    }

    pub async fn drain(&self) -> Vec<Agentsmd> {
        let mut guard = self.0.lock().await;

        guard.pending_instructions.drain(..).collect()
    }

    pub async fn push_hook_contexts(&self, contexts: impl IntoIterator<Item = String>) {
        let mut guard = self.0.lock().await;
        guard.hook_contexts.extend(contexts);
    }

    pub async fn drain_hook_contexts(&self) -> Vec<String> {
        let mut guard = self.0.lock().await;

        guard.hook_contexts.drain(..).collect()
    }
}

#[derive(Clone, Debug, Default)]
struct InstructionState {
    root_dir: PathBuf,
    instructions: Vec<Agentsmd>,
    pending_instructions: VecDeque<Agentsmd>,
    hook_contexts: VecDeque<String>,
}

impl InstructionState {
    pub fn new(root_dir: impl AsRef<Path>) -> Self {
        let root_dir = root_dir.as_ref();

        Self {
            root_dir: root_dir.to_owned(),
            instructions: vec![],
            pending_instructions: VecDeque::new(),
            hook_contexts: VecDeque::new(),
        }
    }
}
