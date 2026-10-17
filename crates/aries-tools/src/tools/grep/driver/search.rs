use std::io;
use std::path::Path;

use grep_regex::RegexMatcher;
use grep_searcher::{
    BinaryDetection, Searcher, SearcherBuilder, Sink, SinkContext, SinkContextKind, SinkMatch,
    sinks,
};

use super::collector::FileMatch;
use super::state::ResultLimit;
use crate::grep::{GrepArgs, OutputMode};

struct ContentSink<'a> {
    path_display: String,
    args: &'a GrepArgs,
    limit: &'a ResultLimit,
    lines: Vec<String>,
    pending_context: Vec<String>,
    after_context_end: u64,
}

impl ContentSink<'_> {
    fn render_line(&self, line_number: u64, bytes: &[u8], is_match: bool) -> String {
        let text = String::from_utf8_lossy(bytes);
        let text = text.strip_suffix('\n').unwrap_or(&text);
        let text = text.strip_suffix('\r').unwrap_or(text);
        let sep = if is_match { ':' } else { '-' };
        if self.args.show_line_numbers {
            format!("{}{sep}{}{sep}{}", self.path_display, line_number, text)
        } else {
            format!("{}{sep}{}", self.path_display, text)
        }
    }
}

impl Sink for ContentSink<'_> {
    type Error = io::Error;

    fn matched(&mut self, _searcher: &Searcher, mat: &SinkMatch<'_>) -> io::Result<bool> {
        let line_number = mat.line_number().expect("line numbers are enabled");
        if !self.limit.try_acquire() {
            if line_number > self.after_context_end {
                return Ok(false);
            }
            self.lines.push(self.render_line(line_number, mat.bytes(), false));
            return Ok(line_number < self.after_context_end);
        }

        self.after_context_end = line_number.saturating_add(self.args.context as u64);
        self.lines.append(&mut self.pending_context);
        self.lines.push(self.render_line(line_number, mat.bytes(), true));
        Ok(true)
    }

    fn context(&mut self, _searcher: &Searcher, ctx: &SinkContext<'_>) -> io::Result<bool> {
        let line_number = ctx.line_number().expect("line numbers are enabled");
        if self.limit.truncated() && line_number > self.after_context_end {
            return Ok(false);
        }
        match ctx.kind() {
            SinkContextKind::Before => {
                self.pending_context.push(self.render_line(line_number, ctx.bytes(), false));
            },
            SinkContextKind::After => {
                self.lines.push(self.render_line(line_number, ctx.bytes(), false));
            },
            SinkContextKind::Other => {},
        }
        Ok(true)
    }
}

pub fn build_searcher(args: &GrepArgs) -> Searcher {
    let mut builder = SearcherBuilder::new();
    builder.line_number(true).binary_detection(BinaryDetection::quit(b'\x00'));
    if args.output_mode == OutputMode::Content {
        builder
            .before_context(args.context)
            .after_context(args.context)
            .heap_limit(Some(10 * 1024 * 1024));
    }
    builder.build()
}

pub fn search_file(
    cwd: &Path,
    args: &GrepArgs,
    matcher: &RegexMatcher,
    searcher: &mut Searcher,
    path: &Path,
    limit: &ResultLimit,
) -> io::Result<Option<FileMatch>> {
    let relative_path = path.strip_prefix(cwd).unwrap_or(path);
    if args.output_mode == OutputMode::Content {
        let mut sink = ContentSink {
            path_display: relative_path.display().to_string(),
            args,
            limit,
            lines: Vec::new(),
            pending_context: Vec::new(),
            after_context_end: 0,
        };
        searcher.search_path(matcher, path, &mut sink)?;
        return Ok((!sink.lines.is_empty()).then_some(FileMatch::Content(sink.lines)));
    }

    let mut count = 0usize;
    searcher.search_path(
        matcher,
        path,
        sinks::Bytes(|_, _| {
            count += 1;
            Ok(args.output_mode == OutputMode::Count)
        }),
    )?;
    if count == 0 {
        return Ok(None);
    }
    if args.output_mode == OutputMode::Count {
        Ok(Some(FileMatch::Count(format!("{}:{count}", relative_path.display()))))
    } else {
        Ok(Some(FileMatch::Path {
            path: relative_path.to_owned(),
            modified: path.metadata()?.modified()?,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_is_the_union_of_context_windows_around_retained_matches() {
        let matcher = RegexMatcher::new("hit").expect("valid regex");
        for mask in 0..64 {
            let lines: Vec<_> =
                (0..6).map(|line| if mask & (1 << line) != 0 { "hit" } else { "miss" }).collect();
            let body = format!("{}\n", lines.join("\n"));
            let matching: Vec<_> = (0..lines.len()).filter(|&line| lines[line] == "hit").collect();
            for limit in [0, 1, 2, 6] {
                let retained = &matching
                    [..if limit == 0 { matching.len() } else { limit.min(matching.len()) }];
                for context in 0..=2 {
                    let args: GrepArgs = serde_json::from_value(serde_json::json!({
                        "pattern": "hit", "output_mode": "content", "limit": limit,
                        "context": context,
                    }))
                    .expect("valid arguments");
                    let quota = ResultLimit::new(limit);
                    let mut sink = ContentSink {
                        path_display: "a.rs".to_owned(),
                        args: &args,
                        limit: &quota,
                        lines: Vec::new(),
                        pending_context: Vec::new(),
                        after_context_end: 0,
                    };
                    build_searcher(&args)
                        .search_slice(&matcher, body.as_bytes(), &mut sink)
                        .expect("search should succeed");

                    let expected: Vec<_> = lines
                        .iter()
                        .enumerate()
                        .filter_map(|(line, text)| {
                            if !retained.iter().any(|&hit| {
                                (hit.saturating_sub(context)..=hit + context).contains(&line)
                            }) {
                                return None;
                            }
                            let sep = if retained.contains(&line) { ':' } else { '-' };
                            Some(format!("a.rs{sep}{}{sep}{text}", line + 1))
                        })
                        .collect();
                    assert_eq!(
                        sink.lines, expected,
                        "mask={mask}, limit={limit}, context={context}"
                    );
                    assert_eq!(quota.truncated(), limit != 0 && matching.len() > limit);
                }
            }
        }
    }
}
