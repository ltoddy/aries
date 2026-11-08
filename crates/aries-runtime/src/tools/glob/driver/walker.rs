use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use ignore::{DirEntry, WalkBuilder, WalkState};

use crate::glob::GlobError;
use crate::tools::glob::driver::Collector;

pub fn walk_parallel(
    base_dir: impl AsRef<Path>,
    hidden: bool,
    respect_ignore: bool,
    set: globset::GlobSet,
    collector: Arc<Collector>,
) -> Result<(), GlobError> {
    let base_dir = base_dir.as_ref();

    let mut builder = WalkBuilder::new(base_dir);
    builder
        .hidden(!hidden)
        .git_ignore(respect_ignore)
        .git_exclude(respect_ignore)
        .git_global(respect_ignore)
        .ignore(respect_ignore);

    let base_dir = Arc::new(base_dir.to_owned());
    let set = Arc::new(set);

    builder.build_parallel().run(|| {
        let mut worker = GlobWorker::new(base_dir.clone(), set.clone(), collector.clone());
        Box::new(move |entry| worker.visit(entry))
    });

    Ok(())
}

struct GlobWorker {
    base_dir: Arc<PathBuf>,
    set: Arc<globset::GlobSet>,
    collector: Arc<Collector>,
    batch: Vec<(PathBuf, SystemTime)>,
}

impl GlobWorker {
    fn new(base_dir: Arc<PathBuf>, set: Arc<globset::GlobSet>, collector: Arc<Collector>) -> Self {
        Self { base_dir, set, collector, batch: Vec::new() }
    }

    fn visit(&mut self, entry: Result<DirEntry, ignore::Error>) -> WalkState {
        let Ok(entry) = entry else {
            return WalkState::Continue;
        };
        if !entry.file_type().is_some_and(|file_type| file_type.is_file()) {
            return WalkState::Continue;
        }

        let Ok(relative) = entry.path().strip_prefix(self.base_dir.as_path()) else {
            return WalkState::Continue;
        };
        if !self.set.is_match(relative) {
            return WalkState::Continue;
        }

        let modified =
            entry.metadata().ok().and_then(|m| m.modified().ok()).unwrap_or(SystemTime::UNIX_EPOCH);

        self.batch.push((relative.to_owned(), modified));
        if self.batch.len() >= 64 {
            self.flush_batch();
        }
        WalkState::Continue
    }

    fn flush_batch(&mut self) {
        if self.batch.is_empty() {
            return;
        }
        self.collector.push(std::mem::take(&mut self.batch));
    }
}

impl Drop for GlobWorker {
    fn drop(&mut self) {
        self.flush_batch();
    }
}
