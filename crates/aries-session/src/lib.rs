mod history;
mod registry;
mod session;

use std::future::Ready;

use rig::agent::MultiTurnStreamItem;

pub use self::registry::SessionRegistry;
pub use self::session::Session;

pub type NoCb = fn(MultiTurnStreamItem<()>) -> Ready<anyhow::Result<()>>;
