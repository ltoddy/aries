mod event;
mod manager;
mod session;

use std::future::Ready;

pub use self::event::{PlanEntry, PlanEntryStatus, StreamEvent};
pub use self::manager::SessionManager;
pub use self::session::Session;

pub type NoCb = fn(StreamEvent) -> Ready<anyhow::Result<()>>;
