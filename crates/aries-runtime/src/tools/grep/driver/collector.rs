use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::path::PathBuf;
use std::time::SystemTime;

use parking_lot::Mutex;

use super::state::ResultLimit;
use crate::grep::{GrepOutput, OutputMode};

pub struct Collector {
    mode: OutputMode,
    pub limit: ResultLimit,
    lines: Mutex<Vec<String>>,
    files: Mutex<BinaryHeap<Reverse<(SystemTime, PathBuf)>>>,
}

impl Collector {
    pub fn new(mode: OutputMode, limit: usize) -> Self {
        Self {
            mode,
            limit: ResultLimit::new(limit),
            lines: Mutex::new(Vec::new()),
            files: Mutex::new(BinaryHeap::new()),
        }
    }

    pub fn push(&self, result: FileMatch) {
        match result {
            FileMatch::Content(lines) => self.lines.lock().extend(lines),
            FileMatch::Count(line) => {
                if self.limit.try_acquire() {
                    self.lines.lock().push(line);
                }
            },
            FileMatch::Path { path, modified } => {
                let entry = Reverse((modified, path));
                let limit = self.limit.limit();
                let mut files = self.files.lock();
                if limit == 0 || files.len() < limit {
                    files.push(entry);
                } else {
                    self.limit.mark_truncated();
                    if let Some(mut oldest) = files.peek_mut()
                        && entry < *oldest
                    {
                        *oldest = entry;
                    }
                }
            },
        }
    }

    pub fn finish(self) -> GrepOutput {
        let matches = match self.mode {
            OutputMode::Content | OutputMode::Count => self.lines.into_inner(),
            OutputMode::FilesWithMatches => self
                .files
                .into_inner()
                .into_sorted_vec()
                .into_iter()
                .map(|Reverse((_, path))| path.display().to_string())
                .collect(),
        };
        GrepOutput::new(matches, self.limit.truncated())
    }
}

pub enum FileMatch {
    Content(Vec<String>),
    Path { path: PathBuf, modified: SystemTime },
    Count(String),
}
