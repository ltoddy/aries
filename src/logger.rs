use std::path::Path;

use tracing_subscriber::prelude::*;
use tracing_subscriber::{EnvFilter, fmt};

pub fn init(dir: &Path) {
    let log_dir = dir.join("logs");
    let file_appender = tracing_appender::rolling::Builder::new()
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .filename_prefix("aries.log")
        .max_log_files(7)
        .build(log_dir)
        .expect("Failed to create log file appender");

    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env())
        .with(fmt::layer().with_writer(non_blocking))
        .with(fmt::layer())
        .init();

    // Store the guard globally to prevent it from being dropped,
    // which would stop the background logging thread.
    Box::leak(Box::new(guard));
}
