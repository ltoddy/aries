use agent_client_protocol::schema::v1 as schema;
use aries_core::tools::update_plan;
use aries_core::tools::update_plan::{PlanEntryPriority, PlanEntryStatus};

pub struct PlanEntry(update_plan::PlanEntry);

impl PlanEntry {
    pub fn new(v: update_plan::PlanEntry) -> Self {
        Self(v)
    }
}

impl From<PlanEntry> for schema::PlanEntry {
    fn from(val: PlanEntry) -> Self {
        let priority = match val.0.priority {
            PlanEntryPriority::High => schema::PlanEntryPriority::High,
            PlanEntryPriority::Medium => schema::PlanEntryPriority::Medium,
            PlanEntryPriority::Low => schema::PlanEntryPriority::Low,
        };

        let status = match val.0.status {
            PlanEntryStatus::Pending => schema::PlanEntryStatus::Pending,
            PlanEntryStatus::InProgress => schema::PlanEntryStatus::InProgress,
            PlanEntryStatus::Completed => schema::PlanEntryStatus::Completed,
        };

        schema::PlanEntry::new(val.0.content, priority, status)
    }
}
