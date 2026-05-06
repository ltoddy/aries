use std::path::Path;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{EnvFilter, fmt};

pub async fn init(dir: impl AsRef<Path>) -> WorkerGuard {
    println!("init 0000");

    let log_dir = dir.as_ref().join("logs");
    if let Some(parent) = log_dir.parent()
        && !parent.exists()
    {
        let _ = tokio::fs::create_dir_all(parent).await;
    }

    let file_appender = tracing_appender::rolling::Builder::new()
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .filename_prefix("aries.log")
        .max_log_files(7)
        .build(log_dir)
        .expect("Failed to create log file appender");
    println!("init 1111");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env())
        .with(fmt::layer().json().with_writer(non_blocking))
        .init();

    println!("init 2222");

    guard
}
