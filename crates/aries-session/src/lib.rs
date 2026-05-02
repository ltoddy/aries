mod history;
mod manager;
mod session;

use std::future::Ready;

use rig::agent::MultiTurnStreamItem;

pub use self::manager::SessionManager;
pub use self::session::Session;

pub type NoCb = fn(MultiTurnStreamItem<()>) -> Ready<anyhow::Result<()>>;
