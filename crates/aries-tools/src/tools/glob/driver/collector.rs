use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::SystemTime;

use parking_lot::Mutex;

pub struct Collector {
    limit: usize,
    entries: Mutex<BinaryHeap<Reverse<(SystemTime, PathBuf)>>>,
    truncated: AtomicBool,
}

impl Collector {
    pub fn new(limit: usize) -> Self {
        Self { limit, entries: Mutex::new(BinaryHeap::new()), truncated: AtomicBool::new(false) }
    }

    pub fn push(&self, batch: Vec<(PathBuf, SystemTime)>) {
        if batch.is_empty() {
            return;
        }

        let mut entries = self.entries.lock();
        if self.limit == 0 {
            entries.extend(batch.into_iter().map(|(path, modified)| Reverse((modified, path))));
            return;
        }

        for (path, modified) in batch {
            let entry = Reverse((modified, path));
            if entries.len() < self.limit {
                entries.push(entry);
                continue;
            }

            if entries.peek().is_some_and(|oldest| entry > *oldest) {
                entries.pop();
                entries.push(entry);
            }
            self.truncated.store(true, Ordering::Relaxed);
        }
    }

    pub fn finish(self) -> (Vec<PathBuf>, bool) {
        let mut entries = self.entries.into_inner();
        let mut sorted = Vec::with_capacity(entries.len());
        while let Some(Reverse((modified, path))) = entries.pop() {
            sorted.push((path, modified));
        }
        sorted.sort_by_key(|(_, modified)| Reverse(*modified));
        let files = sorted.into_iter().map(|(path, _)| path).collect();
        let truncated = self.truncated.load(Ordering::Relaxed);
        (files, truncated)
    }
}
