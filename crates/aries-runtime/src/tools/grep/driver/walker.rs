use std::path::Path;

use globset::Glob;
use grep_regex::RegexMatcher;
use ignore::{WalkBuilder, WalkState};
use parking_lot::Mutex;

use super::collector::Collector;
use super::search::{build_searcher, search_file};
use crate::grep::{GrepArgs, GrepError, OutputMode};

pub fn walk_parallel(
    cwd: &Path,
    args: &GrepArgs,
    matcher: &RegexMatcher,
    collector: &Collector,
) -> Result<(), GrepError> {
    let mut builder = WalkBuilder::new(cwd);
    builder
        .current_dir(cwd)
        .hidden(!args.hidden)
        .parents(args.respect_ignore)
        .git_ignore(args.respect_ignore)
        .git_exclude(args.respect_ignore)
        .git_global(args.respect_ignore)
        .ignore(args.respect_ignore);

    if let Some(pattern) = &args.include {
        let include = Glob::new(pattern.trim_start_matches("./"))
            .map_err(|source| GrepError::include_glob(pattern, source))?
            .compile_matcher();
        let root = cwd.to_owned();
        builder.filter_entry(move |entry| {
            entry.file_type().is_some_and(|kind| kind.is_dir())
                || entry.path().strip_prefix(&root).is_ok_and(|path| include.is_match(path))
        });
    }

    let error = Mutex::new(None);
    builder.build_parallel().run(|| {
        let mut searcher = build_searcher(args);
        let matcher = matcher.clone();
        let error = &error;
        Box::new(move |entry| {
            if args.output_mode != OutputMode::FilesWithMatches && collector.limit.truncated() {
                return WalkState::Quit;
            }
            let result = entry.map_err(GrepError::walk).and_then(|entry| {
                if let Some(error) = entry.error() {
                    return Err(GrepError::walk(error.clone()));
                }
                if !entry.file_type().is_some_and(|kind| kind.is_file()) {
                    return Ok(None);
                }
                search_file(cwd, args, &matcher, &mut searcher, entry.path(), &collector.limit)
                    .map_err(|source| GrepError::search(entry.into_path(), source))
            });
            match result {
                Ok(Some(result)) => collector.push(result),
                Ok(None) => {},
                Err(err) => {
                    error.lock().get_or_insert(err);
                    return WalkState::Quit;
                },
            }
            WalkState::Continue
        })
    });
    error.into_inner().map_or(Ok(()), Err)
}
