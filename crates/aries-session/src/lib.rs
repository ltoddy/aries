pub mod persistence;
pub mod registry;
pub mod session;

use std::future::Ready;

use rig::agent::MultiTurnStreamItem;

pub use self::persistence::{connect, initalize_tables};
pub use self::registry::SessionRegistry;
pub use self::session::Session;

pub type NoCb = fn(MultiTurnStreamItem<()>) -> Ready<anyhow::Result<()>>;
