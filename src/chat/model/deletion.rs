use agent_client_protocol::v1::schema::SessionId;

#[derive(Debug, Default)]
enum DeletionState {
    #[default]
    Idle,
    Confirming(PendingDeletion),
    Deleting,
}

#[derive(Debug, Default)]
pub struct SessionDeletion {
    deletion: DeletionState,
    error: Option<String>,
}

#[derive(Debug)]
pub struct PendingDeletion {
    ids: Vec<SessionId>,
    anchor: String,
}

impl PendingDeletion {
    pub fn is_anchor(&self, anchor: &str) -> bool {
        self.anchor == anchor
    }
}

impl SessionDeletion {
    pub fn is_busy(&self) -> bool {
        !matches!(self.deletion, DeletionState::Idle)
    }

    pub fn is_deleting(&self) -> bool {
        matches!(self.deletion, DeletionState::Deleting)
    }

    pub fn pending_deletion(&self) -> Option<&PendingDeletion> {
        match &self.deletion {
            DeletionState::Confirming(pending) => Some(pending),
            _ => None,
        }
    }

    pub fn prepare_deletion(&mut self, ids: Vec<SessionId>, anchor: String) {
        if ids.is_empty() || self.is_busy() {
            return;
        }
        self.error = None;
        self.deletion = DeletionState::Confirming(PendingDeletion { ids, anchor });
    }

    pub fn begin_deletion(&mut self) -> Option<Vec<SessionId>> {
        match std::mem::take(&mut self.deletion) {
            DeletionState::Confirming(pending) => {
                self.deletion = DeletionState::Deleting;
                Some(pending.ids)
            }
            other => {
                self.deletion = other;
                None
            }
        }
    }

    pub fn cancel_deletion(&mut self) {
        if matches!(self.deletion, DeletionState::Confirming(_)) {
            self.deletion = DeletionState::Idle;
        }
    }

    pub fn fail_deletion(&mut self, error: String) {
        self.deletion = DeletionState::Idle;
        self.error = Some(error);
    }

    pub fn finish_deletion(&mut self, error: Option<String>) {
        self.deletion = DeletionState::Idle;
        self.error = error;
    }

    pub fn disconnect(&mut self) {
        if self.is_deleting() {
            self.fail_deletion("删除期间连接中断，请重新获取列表确认结果。".into());
        } else {
            self.cancel_deletion();
        }
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
}
