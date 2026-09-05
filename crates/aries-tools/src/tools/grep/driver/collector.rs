use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use parking_lot::Mutex;

use crate::grep::OutputMode;
use crate::grep::driver::StopState;

#[derive(Default)]
pub struct Collector {
    mode: OutputMode,
    stop: Arc<StopState>,
    content_groups: Mutex<Vec<Vec<String>>>,
    file_entries: Mutex<Vec<(PathBuf, SystemTime)>>,
    count_lines: Mutex<Vec<String>>,
}

impl Collector {
    pub fn new(mode: OutputMode, stop: Arc<StopState>) -> Self {
        Self { mode, stop, ..Default::default() }
    }
    pub fn push(&self, ThreadBatch { batch }: ThreadBatch) {
        if batch.is_empty() {
            return;
        }

        match self.mode {
            OutputMode::Content => self.on_content(batch),
            OutputMode::FilesWithMatches => self.on_files_with_matches(batch),
            OutputMode::Count => self.on_count(batch),
        }
    }

    pub fn should_stop(&self) -> bool {
        self.stop.should_stop()
    }

    pub fn finish(self) -> (Vec<String>, bool) {
        let matches = match self.mode {
            OutputMode::Content => self.content_groups.into_inner().into_iter().flatten().collect(),
            OutputMode::FilesWithMatches => {
                let mut entries = self.file_entries.into_inner();
                entries.sort_by_key(|(_, modified)| std::cmp::Reverse(*modified));
                if self.stop.limit() > 0 {
                    entries.truncate(self.stop.limit());
                }
                entries.into_iter().map(|(path, _)| path.display().to_string()).collect()
            },
            OutputMode::Count => self.count_lines.into_inner(),
        };
        (matches, self.stop.truncated())
    }

    fn on_content(&self, batch: Vec<FileMatch>) {
        let mut groups = batch
            .into_iter()
            .filter_map(
                |result| {
                    if let FileMatch::Content(lines) = result { Some(lines) } else { None }
                },
            )
            .collect::<Vec<_>>();

        let granted = self.stop.grant(groups.len());
        if granted == 0 {
            return;
        }
        groups.truncate(granted);
        self.content_groups.lock().extend(groups);
    }

    fn on_files_with_matches(&self, batch: Vec<FileMatch>) {
        let file_entries = batch
            .into_iter()
            .filter_map(|result| {
                if let FileMatch::Path { path, modified } = result {
                    Some((path, modified))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        self.file_entries.lock().extend(file_entries);
    }

    fn on_count(&self, batch: Vec<FileMatch>) {
        let mut count_lines = batch
            .into_iter()
            .filter_map(
                |result| if let FileMatch::Count(line) = result { Some(line) } else { None },
            )
            .collect::<Vec<_>>();

        let granted = self.stop.grant(count_lines.len());
        if granted == 0 {
            return;
        }
        count_lines.truncate(granted);
        self.count_lines.lock().extend(count_lines);
    }
}

pub enum FileMatch {
    Content(Vec<String>),
    Path { path: PathBuf, modified: SystemTime },
    Count(String),
}

impl FileMatch {
    pub fn content(lines: Vec<String>) -> Self {
        Self::Content(lines)
    }

    pub fn path(path: impl AsRef<Path>, modified: SystemTime) -> Self {
        let path = path.as_ref();
        Self::Path { path: path.to_owned(), modified }
    }

    pub fn count(line: String) -> Self {
        Self::Count(line)
    }
}

#[derive(Default)]
pub struct ThreadBatch {
    batch: Vec<FileMatch>,
}

impl ThreadBatch {
    pub fn push(&mut self, result: FileMatch) {
        self.batch.push(result);
    }

    pub fn is_empty(&self) -> bool {
        self.batch.is_empty()
    }
}
