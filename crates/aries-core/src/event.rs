use rig_core::agent::MultiTurnStreamItem;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct AgentEvent<R> {
    name: String, // agent name
    item: MultiTurnStreamItem<R>,
}

impl<R> AgentEvent<R> {
    pub fn new(name: impl Into<String>, item: MultiTurnStreamItem<R>) -> Self {
        let name = name.into();
        Self { name, item }
    }
}
