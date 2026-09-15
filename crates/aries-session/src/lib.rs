mod commands;
mod middleware;
mod provider;
mod registry;
mod session;

use std::sync::Arc;

use tokio::sync::Mutex;

pub use self::commands::BUILTIN_COMMANDS;
pub use self::provider::AriesClientProvider;
pub use self::registry::SessionRegistry;
pub use self::session::{Session, SessionArgs, resume_input};

pub type SharedRegistry = Arc<Mutex<SessionRegistry>>;
