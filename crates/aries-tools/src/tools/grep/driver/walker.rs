use std::path::{Path, PathBuf};
use std::sync::Arc;

use globset::{Glob, GlobSet, GlobSetBuilder};
use grep_regex::RegexMatcher;
use ignore::{DirEntry, WalkBuilder, WalkState};

use crate::grep::GrepError;
use crate::tools::grep::driver::Query;
use crate::tools::grep::driver::collector::{Collector, ThreadBatch};
use crate::tools::grep::driver::search::search_file;

pub fn walk_parallel(
    cwd: impl AsRef<Path>,
    query: Query,
    matcher: RegexMatcher,
    collector: Arc<Collector>,
) -> Result<(), GrepError> {
    let cwd = cwd.as_ref();

    let entry_matcher = EntryMatcher::new(cwd, &query)?;
    let mut builder = WalkBuilder::new(cwd);
    entry_matcher.configure(&mut builder, &query);

    builder.build_parallel().run(|| {
        let mut worker = SearchWorker::new(cwd, query.clone(), matcher.clone(), collector.clone());
        Box::new(move |entry| worker.search(entry))
    });

    Ok(())
}

struct SearchWorker {
    cwd: PathBuf,
    query: Query,
    matcher: RegexMatcher,
    collector: Arc<Collector>,
    batch: ThreadBatch,
}

impl SearchWorker {
    fn new(
        cwd: impl AsRef<Path>,
        query: Query,
        matcher: RegexMatcher,
        collector: Arc<Collector>,
    ) -> Self {
        let cwd = cwd.as_ref();

        Self { cwd: cwd.to_owned(), query, matcher, collector, batch: ThreadBatch::default() }
    }

    fn search(&mut self, entry: Result<DirEntry, ignore::Error>) -> WalkState {
        if self.collector.should_stop()
            && !matches!(self.query.output_mode, crate::grep::OutputMode::FilesWithMatches)
        {
            self.flush_batch();
            return WalkState::Quit;
        }

        let Ok(entry) = entry else {
            return WalkState::Continue;
        };
        if !entry.file_type().is_some_and(|file_type| file_type.is_file()) {
            return WalkState::Continue;
        }

        if let Some(result) = search_file(&self.cwd, &self.query, &self.matcher, entry.path()) {
            self.batch.push(result);
        }

        self.flush_batch();
        if self.collector.should_stop()
            && !matches!(self.query.output_mode, crate::grep::OutputMode::FilesWithMatches)
        {
            WalkState::Quit
        } else {
            WalkState::Continue
        }
    }

    fn flush_batch(&mut self) {
        if self.batch.is_empty() {
            return;
        }
        let flushed = std::mem::take(&mut self.batch);
        self.collector.push(flushed);
    }
}

#[derive(Clone)]
struct EntryMatcher {
    cwd: PathBuf,
    include_filter: Option<IncludeFilter>,
}

impl EntryMatcher {
    fn new(cwd: impl AsRef<Path>, query: &Query) -> Result<Self, GrepError> {
        let cwd = cwd.as_ref();

        let include_filter = match &query.include {
            Some(include) => Some(IncludeFilter::new(include)?),
            None => None,
        };

        Ok(Self { cwd: cwd.to_owned(), include_filter })
    }

    fn matches(&self, entry: &DirEntry) -> bool {
        let Some(filter) = self.include_filter.as_ref() else {
            return true;
        };

        let relative_path = match entry.path().strip_prefix(&self.cwd) {
            Ok(path) => path,
            Err(_) => return false,
        };

        if entry.file_type().is_some_and(|file_type| file_type.is_dir()) {
            return filter.matches_dir(relative_path);
        }

        filter.matches_file(relative_path)
    }

    fn configure(&self, builder: &mut WalkBuilder, query: &Query) {
        builder
            .hidden(false)
            .git_ignore(query.respect_gitignore)
            .git_exclude(query.respect_gitignore)
            .git_global(query.respect_gitignore)
            .ignore(query.respect_gitignore);

        let matcher = self.clone();
        builder.filter_entry(move |entry| matcher.matches(entry));
    }
}

#[derive(Clone)]
struct IncludeFilter {
    files: GlobSet,
    directories: GlobSet,
}

impl IncludeFilter {
    fn new(include: impl AsRef<str>) -> Result<Self, GrepError> {
        let include = include.as_ref();

        let mut file_builder = GlobSetBuilder::new();
        file_builder.add(Glob::new(include)?);

        let mut directory_builder = GlobSetBuilder::new();
        for prefix in directory_prefix_patterns(include) {
            directory_builder.add(Glob::new(&prefix)?);
        }

        Ok(Self { files: file_builder.build()?, directories: directory_builder.build()? })
    }

    fn matches_file(&self, path: &Path) -> bool {
        self.files.is_match(path)
    }

    fn matches_dir(&self, path: &Path) -> bool {
        path.as_os_str().is_empty() || self.directories.is_match(path)
    }
}

pub(super) fn directory_prefix_patterns(include: impl AsRef<str>) -> Vec<String> {
    let include = include.as_ref();
    let normalized = include.trim_start_matches("./");
    let mut prefixes = vec!["**".to_string()];
    let mut current = PathBuf::new();

    for component in Path::new(normalized).components() {
        let part = component.as_os_str().to_string_lossy();
        if part.contains('*') || part.contains('?') || part.contains('[') || part == "**" {
            break;
        }
        current.push(part.as_ref());
        prefixes.push(format!("{}", current.display()));
        prefixes.push(format!("{}/**", current.display()));
    }

    prefixes.sort();
    prefixes.dedup();
    prefixes
}
