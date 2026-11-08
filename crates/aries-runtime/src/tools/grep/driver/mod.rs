mod collector;
mod search;
mod state;
mod walker;

use std::path::Path;

use grep_regex::RegexMatcherBuilder;

use self::collector::Collector;
use crate::grep::{GrepArgs, GrepError, GrepOutput};

pub fn run(cwd: &Path, args: &GrepArgs) -> Result<GrepOutput, GrepError> {
    let matcher = RegexMatcherBuilder::new()
        .case_insensitive(args.case_insensitive)
        .multi_line(true)
        .line_terminator(Some(b'\n'))
        .build(&args.pattern)
        .map_err(|source| GrepError::regex(&args.pattern, source))?;
    let collector = Collector::new(args.output_mode, args.limit);
    walker::walk_parallel(cwd, args, &matcher, &collector)?;
    Ok(collector.finish())
}
