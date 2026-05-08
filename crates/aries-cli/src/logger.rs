use std::path::Path;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{EnvFilter, fmt};

pub async fn init(dir: impl AsRef<Path>) -> WorkerGuard {
    let log_dir = dir.as_ref().join("logs");

    if !log_dir.exists() {
        let _ = tokio::fs::create_dir_all(&log_dir).await;
    }

    let file_appender = tracing_appender::rolling::Builder::new()
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .filename_prefix("aries.log")
        .build(log_dir)
        .expect("Failed to create log file appender");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env())
        .with(fmt::layer().json().with_writer(non_blocking))
        .init();

    guard
}
