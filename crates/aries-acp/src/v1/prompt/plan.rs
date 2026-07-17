use agent_client_protocol::schema::v1 as schema;
use aries_tools::update_plan::{self, PlanEntryPriority, PlanEntryStatus};

pub struct PlanEntry(update_plan::PlanEntry);

impl PlanEntry {
    pub fn new(v: update_plan::PlanEntry) -> Self {
        Self(v)
    }
}

impl From<PlanEntry> for schema::PlanEntry {
    fn from(PlanEntry(v): PlanEntry) -> Self {
        let update_plan::PlanEntry { content, active_form, priority, status } = v;

        let priority = match priority {
            PlanEntryPriority::High => schema::PlanEntryPriority::High,
            PlanEntryPriority::Medium => schema::PlanEntryPriority::Medium,
            PlanEntryPriority::Low => schema::PlanEntryPriority::Low,
        };

        let status = match status {
            PlanEntryStatus::Pending => schema::PlanEntryStatus::Pending,
            PlanEntryStatus::InProgress => schema::PlanEntryStatus::InProgress,
            PlanEntryStatus::Completed => schema::PlanEntryStatus::Completed,
        };

        let mut meta = serde_json::Map::new();
        meta.insert("activeForm".to_owned(), serde_json::Value::String(active_form));

        schema::PlanEntry::new(content, priority, status).meta(meta)
    }
}
