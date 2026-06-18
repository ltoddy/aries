use std::path::Path;
use std::sync::Arc;

use anyhow::Context;
use tracing::Dispatch;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{EnvFilter, fmt};

#[derive(Clone)]
pub struct Logger {
    dispatch: Dispatch,
    _guard: Arc<WorkerGuard>,
}

impl Logger {
    pub async fn new(session_dir: impl AsRef<Path>) -> anyhow::Result<Self> {
        let session_dir = session_dir.as_ref();
        let log_dir = session_dir.join("logs");

        let file_appender = tracing_appender::rolling::Builder::new()
            .rotation(tracing_appender::rolling::Rotation::DAILY)
            .filename_prefix("aries.log")
            .build(&log_dir)
            .with_context(|| {
                format!("failed to create log file appender at: {}", log_dir.display())
            })?;
        let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

        let subscriber = tracing_subscriber::registry()
            .with(EnvFilter::new("info"))
            .with(fmt::layer().json().with_writer(non_blocking));

        Ok(Self { dispatch: Dispatch::new(subscriber), _guard: Arc::new(guard) })
    }

    pub fn dispatch(&self) -> Dispatch {
        self.dispatch.clone()
    }
}
