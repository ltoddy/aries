use gpui_kit::EntityId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PetActivity {
    Idle,
    Working,
    Waiting,
    Failed,
}

impl PetActivity {
    pub fn idle() -> Self {
        Self::Idle
    }

    pub fn working() -> Self {
        Self::Working
    }

    pub fn waiting() -> Self {
        Self::Waiting
    }

    pub fn failed() -> Self {
        Self::Failed
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PetSession {
    id: EntityId,
    activity: PetActivity,
    completion: u64,
}

impl PetSession {
    pub fn new(id: EntityId, activity: PetActivity, completion: u64) -> Self {
        Self { id, activity, completion }
    }

    pub fn id(&self) -> EntityId {
        self.id
    }

    pub fn activity(&self) -> PetActivity {
        self.activity
    }

    pub fn completion(&self) -> u64 {
        self.completion
    }
}
