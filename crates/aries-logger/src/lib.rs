use std::path::Path;
use std::sync::OnceLock;

use tracing::level_filters::LevelFilter;
use tracing_subscriber::prelude::*;

use crate::layer::RouterLayer;
use crate::router::LogRouter;

mod layer;
mod router;

static ROUTER: OnceLock<LogRouter> = OnceLock::new();

pub fn init(dir: impl AsRef<Path>) {
    let dir = dir.as_ref();

    let router = ROUTER.get_or_init(|| LogRouter::new(dir));
    let lyr = RouterLayer { inner: router.inner.clone() };
    let _ = tracing_subscriber::registry().with(LevelFilter::INFO).with(lyr).try_init();
}

pub fn register(session_id: impl Into<String>, dir: impl AsRef<Path>) {
    if let Some(router) = ROUTER.get() {
        router.register(session_id, dir);
    }
}
