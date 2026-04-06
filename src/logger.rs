use std::path::Path;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{EnvFilter, fmt};

pub fn init(dir: &Path) -> WorkerGuard {
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
        .with(fmt::layer().json().with_writer(non_blocking))
        .with(fmt::layer())
        .init();

    guard
}
