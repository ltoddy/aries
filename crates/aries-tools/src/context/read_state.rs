use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use parking_lot::Mutex;

#[derive(Clone, Copy, Debug)]
pub struct ReadRecord {
    pub timestamp_millis: u128,
}

impl ReadRecord {
    pub fn new(timestamp_millis: u128) -> Self {
        Self { timestamp_millis }
    }
}

#[derive(Clone, Debug)]
pub struct SharedReadState(Arc<Mutex<HashMap<PathBuf, ReadRecord>>>);

impl Default for SharedReadState {
    fn default() -> Self {
        Self::new()
    }
}

impl SharedReadState {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(HashMap::with_capacity(32))))
    }

    pub fn record(&self, file_path: impl Into<PathBuf>, timestamp_millis: u128) {
        let file_path = file_path.into();

        let mut guard = self.0.lock();
        guard.insert(file_path, ReadRecord::new(timestamp_millis));
    }

    pub fn get(&self, file_path: impl AsRef<Path>) -> Option<ReadRecord> {
        let guard = self.0.lock();
        guard.get(file_path.as_ref()).copied()
    }
}
