use std::path::Path;

use tracing_subscriber::prelude::*;
use tracing_subscriber::{EnvFilter, fmt};

pub fn init(_dir: &Path) {
    tracing_subscriber::registry().with(EnvFilter::from_default_env()).with(fmt::layer()).init();
}
