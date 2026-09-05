mod collector;
mod search;
mod state;
mod walker;

use grep_searcher::MmapChoice;

pub use self::collector::Collector;
pub use self::state::StopState;
pub use self::walker::walk_parallel;
use crate::grep::{GrepArgs, OutputMode};

#[derive(Clone)]
pub struct Query {
    pub include: Option<String>,
    pub output_mode: OutputMode,
    pub show_line_numbers: bool,
    pub context_before: Option<usize>,
    pub context_after: Option<usize>,
    pub context: Option<usize>,
    pub hidden: bool,
    pub respect_ignore: bool,
}

impl From<GrepArgs> for Query {
    fn from(value: GrepArgs) -> Self {
        Self {
            include: value.include,
            output_mode: value.output_mode,
            show_line_numbers: value.show_line_numbers,
            context_before: value.context_before,
            context_after: value.context_after,
            context: value.context,
            hidden: value.hidden,
            respect_ignore: value.respect_ignore,
        }
    }
}

impl Query {
    pub fn context_before(&self) -> usize {
        self.context.unwrap_or(self.context_before.unwrap_or(0))
    }

    pub fn context_after(&self) -> usize {
        self.context.unwrap_or(self.context_after.unwrap_or(0))
    }

    pub fn mmap_choice(&self) -> MmapChoice {
        match self.output_mode {
            OutputMode::Content => MmapChoice::never(),
            OutputMode::FilesWithMatches | OutputMode::Count => unsafe { MmapChoice::auto() },
        }
    }

    pub fn heap_limit(&self) -> Option<usize> {
        match self.output_mode {
            OutputMode::Content => Some(10 * 1024 * 1024),
            OutputMode::FilesWithMatches | OutputMode::Count => None,
        }
    }
}
