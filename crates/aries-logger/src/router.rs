use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use parking_lot::Mutex;
use tracing_appender::non_blocking::{NonBlocking, WorkerGuard};

pub struct SessionWriter {
    pub _guard: WorkerGuard,
    pub writer: NonBlocking,
}

impl SessionWriter {
    pub fn new(_guard: WorkerGuard, writer: NonBlocking) -> Self {
        Self { _guard, writer }
    }
}

pub struct Inner {
    pub current_writer: SessionWriter,
    pub sessions: HashMap<String, SessionWriter>,
}

pub struct LogRouter {
    pub inner: Arc<Mutex<Inner>>,
}

impl LogRouter {
    pub fn new(dir: impl AsRef<Path>) -> LogRouter {
        let dir = dir.as_ref();

        let _ = std::fs::create_dir_all(dir);

        let file_appender = tracing_appender::rolling::Builder::new()
            .rotation(tracing_appender::rolling::Rotation::DAILY)
            .filename_prefix("aries.log")
            .build(dir)
            .expect("failed to create default log file appender");
        let (writer, _guard) = tracing_appender::non_blocking(file_appender);

        let inner =
            Inner { current_writer: SessionWriter::new(_guard, writer), sessions: HashMap::new() };

        LogRouter { inner: Arc::new(Mutex::new(inner)) }
    }

    pub fn register(&self, session_id: impl Into<String>, dir: impl AsRef<Path>) {
        let dir = dir.as_ref();
        let session_id = session_id.into();

        let log_dir = dir.join("logs");
        let _ = std::fs::create_dir_all(&log_dir);

        let file_appender = tracing_appender::rolling::Builder::new()
            .rotation(tracing_appender::rolling::Rotation::DAILY)
            .filename_prefix("aries.log")
            .build(log_dir)
            .expect("failed to create session log file appender");
        let (writer, _guard) = tracing_appender::non_blocking(file_appender);

        self.inner.lock().sessions.insert(session_id, SessionWriter::new(_guard, writer));
    }
}
