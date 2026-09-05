use std::path::Path;
use std::time::SystemTime;

use grep_regex::RegexMatcher;
use grep_searcher::{
    BinaryDetection, Searcher, SearcherBuilder, Sink, SinkContext, SinkContextKind, SinkMatch,
};

use crate::grep::OutputMode;
use crate::tools::grep::driver::Query;
use crate::tools::grep::driver::collector::FileMatch;

struct ContentSink {
    path_display: String,
    show_line_numbers: bool,
    lines: Vec<String>,
}

impl ContentSink {
    fn new(path_display: impl Into<String>, show_line_numbers: bool) -> Self {
        Self { path_display: path_display.into(), show_line_numbers, lines: Vec::new() }
    }

    fn push_line(&mut self, line_number: u64, bytes: &[u8], is_match: bool) {
        let text = String::from_utf8_lossy(bytes);
        let text = text.strip_suffix('\n').unwrap_or(&text);
        let text = text.strip_suffix('\r').unwrap_or(text);
        let sep = if is_match { ':' } else { '-' };
        let rendered = if self.show_line_numbers {
            format!("{}{sep}{}{sep}{}", self.path_display, line_number, text)
        } else {
            format!("{}{sep}{}", self.path_display, text)
        };
        self.lines.push(rendered);
    }
}

impl Sink for ContentSink {
    type Error = std::io::Error;

    fn matched(&mut self, _searcher: &Searcher, mat: &SinkMatch<'_>) -> Result<bool, Self::Error> {
        self.push_line(mat.line_number().unwrap_or(1), mat.bytes(), true);
        Ok(true)
    }

    fn context(
        &mut self,
        _searcher: &Searcher,
        ctx: &SinkContext<'_>,
    ) -> Result<bool, Self::Error> {
        match ctx.kind() {
            SinkContextKind::Before | SinkContextKind::After => {
                self.push_line(ctx.line_number().unwrap_or(1), ctx.bytes(), false);
            },
            SinkContextKind::Other => {},
        }
        Ok(true)
    }
}

struct MatchFoundSink {
    found: bool,
}

impl MatchFoundSink {
    fn new() -> Self {
        Self { found: false }
    }
}

impl Sink for MatchFoundSink {
    type Error = std::io::Error;

    fn matched(&mut self, _searcher: &Searcher, _mat: &SinkMatch<'_>) -> Result<bool, Self::Error> {
        self.found = true;
        Ok(false)
    }
}

struct CountSink {
    count: usize,
}

impl CountSink {
    fn new() -> Self {
        Self { count: 0 }
    }
}

impl Sink for CountSink {
    type Error = std::io::Error;

    fn matched(&mut self, _searcher: &Searcher, _mat: &SinkMatch<'_>) -> Result<bool, Self::Error> {
        self.count += 1;
        Ok(true)
    }
}

pub fn search_file(
    cwd: impl AsRef<Path>,
    query: &Query,
    matcher: &RegexMatcher,
    path: impl AsRef<Path>,
) -> Option<FileMatch> {
    let path = path.as_ref();
    let relative_path = path.strip_prefix(cwd).unwrap_or(path);
    let path_display = relative_path.display().to_string();

    match query.output_mode {
        OutputMode::Content => {
            let mut builder = SearcherBuilder::new();
            builder
                .line_number(true)
                .before_context(query.context_before())
                .after_context(query.context_after())
                .binary_detection(BinaryDetection::quit(b'\x00'))
                .memory_map(query.mmap_choice())
                .heap_limit(query.heap_limit());
            let mut searcher = builder.build();
            let mut sink = ContentSink::new(path_display, query.show_line_numbers);
            if searcher.search_path(matcher, path, &mut sink).is_err() {
                return None;
            }
            if sink.lines.is_empty() { None } else { Some(FileMatch::content(sink.lines)) }
        },
        OutputMode::FilesWithMatches => {
            let mut builder = SearcherBuilder::new();
            builder
                .line_number(true)
                .binary_detection(BinaryDetection::quit(b'\x00'))
                .memory_map(query.mmap_choice())
                .heap_limit(query.heap_limit());
            let mut searcher = builder.build();
            let mut sink = MatchFoundSink::new();
            if searcher.search_path(matcher, path, &mut sink).is_err() {
                return None;
            }
            if !sink.found {
                return None;
            }
            let modified = path
                .metadata()
                .ok()
                .and_then(|metadata| metadata.modified().ok())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            Some(FileMatch::path(relative_path, modified))
        },
        OutputMode::Count => {
            let mut builder = SearcherBuilder::new();
            builder
                .line_number(true)
                .binary_detection(BinaryDetection::quit(b'\x00'))
                .memory_map(query.mmap_choice())
                .heap_limit(query.heap_limit());
            let mut searcher = builder.build();
            let mut sink = CountSink::new();
            if searcher.search_path(matcher, path, &mut sink).is_err() {
                return None;
            }
            if sink.count == 0 {
                None
            } else {
                Some(FileMatch::count(format!("{path_display}:{}", sink.count)))
            }
        },
    }
}
