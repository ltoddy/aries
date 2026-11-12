use std::ops::RangeInclusive;

use anyhow::{Context, Result, bail, ensure};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PatchDirection {
    Forward,
    Reverse,
}

impl PatchDirection {
    pub fn forward() -> Self {
        Self::Forward
    }

    pub fn reverse() -> Self {
        Self::Reverse
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceSide {
    Original,
    Modified,
}

impl SourceSide {
    pub fn original() -> Self {
        Self::Original
    }

    pub fn modified() -> Self {
        Self::Modified
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LineEndpoint {
    side: SourceSide,
    line: u32,
}

impl LineEndpoint {
    pub fn new(side: SourceSide, line: u32) -> Result<Self> {
        ensure!(line > 0, "line numbers are one-based");
        Ok(Self { side, line })
    }

    pub fn side(self) -> SourceSide {
        self.side
    }

    pub fn line(self) -> u32 {
        self.line
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LineSelection {
    source_range: RangeInclusive<usize>,
    side: Option<SourceSide>,
}

impl LineSelection {
    pub fn from_endpoints(
        patch: &DiffPatch,
        anchor: LineEndpoint,
        head: LineEndpoint,
    ) -> Result<Self> {
        let side = (anchor.side() == head.side()).then_some(anchor.side());
        let anchor =
            patch.source_position(anchor).context("selection anchor is outside the patch")?;
        let head = patch.source_position(head).context("selection head is outside the patch")?;
        let mut selection = Self::new(anchor.min(head)..=anchor.max(head));
        selection.side = side;
        Ok(selection)
    }

    pub fn new(source_range: RangeInclusive<usize>) -> Self {
        Self { source_range, side: None }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PatchScope {
    File,
    Hunk(usize),
    Lines(LineSelection),
}

impl PatchScope {
    pub fn file() -> Self {
        Self::File
    }

    pub fn hunk(index: usize) -> Self {
        Self::Hunk(index)
    }

    pub fn lines(selection: LineSelection) -> Self {
        Self::Lines(selection)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceLine {
    position: usize,
    hunk: usize,
    origin: char,
    original_line: Option<u32>,
    modified_line: Option<u32>,
    content: String,
    no_newline: bool,
}

impl SourceLine {
    fn new(
        position: usize,
        hunk: usize,
        origin: char,
        coordinates: (Option<u32>, Option<u32>),
        content: String,
    ) -> Self {
        Self {
            position,
            hunk,
            origin,
            original_line: coordinates.0,
            modified_line: coordinates.1,
            content,
            no_newline: false,
        }
    }

    pub fn position(&self) -> usize {
        self.position
    }

    pub fn hunk_index(&self) -> usize {
        self.hunk
    }

    pub fn origin(&self) -> char {
        self.origin
    }

    pub fn original_line(&self) -> Option<u32> {
        self.original_line
    }

    pub fn modified_line(&self) -> Option<u32> {
        self.modified_line
    }

    pub fn content(&self) -> &str {
        &self.content
    }

    pub fn has_no_newline(&self) -> bool {
        self.no_newline
    }

    pub fn is_changed(&self) -> bool {
        matches!(self.origin(), '+' | '-')
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PatchHunk {
    index: usize,
    label: String,
    original_start: u32,
    original_count: u32,
    modified_start: u32,
    modified_count: u32,
    source_range: RangeInclusive<usize>,
}

impl PatchHunk {
    fn new(index: usize, label: String, old: (u32, u32), new: (u32, u32), position: usize) -> Self {
        Self {
            index,
            label,
            original_start: old.0,
            original_count: old.1,
            modified_start: new.0,
            modified_count: new.1,
            source_range: position..=position,
        }
    }

    pub fn index(&self) -> usize {
        self.index
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn original_start(&self) -> u32 {
        self.original_start
    }

    pub fn original_count(&self) -> u32 {
        self.original_count
    }

    pub fn modified_start(&self) -> u32 {
        self.modified_start
    }

    pub fn modified_count(&self) -> u32 {
        self.modified_count
    }
}

#[derive(Clone, Debug)]
pub struct DiffPatch {
    raw: String,
    header: Vec<String>,
    hunks: Vec<PatchHunk>,
    source_lines: Vec<SourceLine>,
    partial_error: Option<String>,
}

impl DiffPatch {
    fn new(raw: String) -> Self {
        Self {
            raw,
            header: Vec::new(),
            hunks: Vec::new(),
            source_lines: Vec::new(),
            partial_error: None,
        }
    }

    pub fn parse(raw: &str) -> Result<Self> {
        ensure!(!raw.is_empty(), "patch is empty");
        let validated = git2::Diff::from_buffer(raw.as_bytes()).context("invalid Git patch")?;
        ensure!(validated.deltas().len() == 1, "expected exactly one file patch");
        let mut patch = Self::new(raw.to_owned());
        if let Some(delta) = validated.get_delta(0) {
            if delta.flags().is_binary() {
                patch.partial_error =
                    Some("binary patches do not support partial operations".to_owned());
            }
            if !matches!(
                delta.status(),
                git2::Delta::Added
                    | git2::Delta::Deleted
                    | git2::Delta::Modified
                    | git2::Delta::Untracked
            ) {
                patch.partial_error =
                    Some("rename and type changes do not support partial operations".to_owned());
            }
            for mode in [delta.old_file().mode(), delta.new_file().mode()] {
                if !matches!(
                    mode,
                    git2::FileMode::Unreadable
                        | git2::FileMode::Blob
                        | git2::FileMode::BlobExecutable
                ) {
                    patch.partial_error = Some(
                        "symlinks and submodules do not support partial operations".to_owned(),
                    );
                }
            }
        }
        let mut old = 0;
        let mut new = 0;
        let mut hunk_start = 0;
        for (position, line) in raw.split_inclusive('\n').enumerate() {
            if line.starts_with("@@ ") {
                if let Some(hunk) = patch.hunks.last() {
                    Self::validate_hunk(hunk, &patch.source_lines[hunk_start..])?;
                }
                hunk_start = patch.source_lines.len();
                let (before, after, label) = parse_hunk_header(line)?;
                old = before.0;
                new = after.0;
                patch.hunks.push(PatchHunk::new(patch.hunks.len(), label, before, after, position));
                continue;
            }
            if patch.hunks.is_empty() {
                patch.header.push(line.to_owned());
                if line.starts_with("old mode ")
                    || line.starts_with("new mode ")
                    || line.starts_with("rename ")
                    || line.starts_with("similarity ")
                    || line.starts_with("Binary ")
                    || line.starts_with("GIT binary patch")
                {
                    patch.partial_error = Some(
                        "metadata and binary changes do not support partial operations".to_owned(),
                    );
                }
                continue;
            }
            if line == "\\ No newline at end of file\n" || line == "\\ No newline at end of file" {
                let previous = patch.source_lines.last_mut().context("orphan EOF marker")?;
                ensure!(!previous.no_newline, "duplicate EOF marker");
                previous.no_newline = true;
                continue;
            }
            let origin = line.chars().next().context("empty patch line")?;
            ensure!(matches!(origin, ' ' | '+' | '-'), "unsupported patch body");
            ensure!(line.ends_with('\n'), "patch body line is not terminated");
            let coordinates = match origin {
                ' ' => (Some(old), Some(new)),
                '-' => (Some(old), None),
                '+' => (None, Some(new)),
                _ => unreachable!(),
            };
            patch.source_lines.push(SourceLine::new(
                position,
                patch.hunks.len() - 1,
                origin,
                coordinates,
                line[1..].to_owned(),
            ));
            if origin != '+' {
                old = old.checked_add(1).context("original coordinate overflow")?;
            }
            if origin != '-' {
                new = new.checked_add(1).context("modified coordinate overflow")?;
            }
            let hunk = patch.hunks.last_mut().context("missing hunk")?;
            hunk.source_range = *hunk.source_range.start()..=position;
        }
        if let Some(hunk) = patch.hunks.last() {
            Self::validate_hunk(hunk, &patch.source_lines[hunk_start..])?;
        }
        if patch.hunks.is_empty() && patch.partial_error.is_none() {
            patch.partial_error = Some("patch has no text hunks".to_owned());
        }
        Ok(patch)
    }

    fn validate_hunk(hunk: &PatchHunk, lines: &[SourceLine]) -> Result<()> {
        let old = lines.iter().filter(|line| line.origin != '+').count();
        let new = lines.iter().filter(|line| line.origin != '-').count();
        ensure!(
            old == hunk.original_count as usize && new == hunk.modified_count as usize,
            "hunk counts do not match body"
        );
        Ok(())
    }

    pub fn hunks(&self) -> &[PatchHunk] {
        &self.hunks
    }

    pub fn source_lines(&self) -> &[SourceLine] {
        &self.source_lines
    }

    pub fn partial_error(&self) -> Option<&str> {
        self.partial_error.as_deref()
    }

    pub fn is_discardable(&self) -> bool {
        self.partial_error.is_none()
            || (self.partial_error.as_deref() == Some("patch has no text hunks")
                && self.header.iter().any(|line| {
                    matches!(
                        line.trim(),
                        "new file mode 100644"
                            | "new file mode 100755"
                            | "deleted file mode 100644"
                            | "deleted file mode 100755"
                    )
                }))
    }

    pub fn source_position(&self, endpoint: LineEndpoint) -> Option<usize> {
        self.source_lines
            .iter()
            .find(|line| match endpoint.side {
                SourceSide::Original => line.original_line == Some(endpoint.line()),
                SourceSide::Modified => line.modified_line == Some(endpoint.line()),
            })
            .map(SourceLine::position)
    }

    fn validate_scope(&self, scope: &PatchScope) -> Result<()> {
        match scope {
            PatchScope::File => {},
            PatchScope::Hunk(index) => ensure!(*index < self.hunks.len(), "hunk no longer exists"),
            PatchScope::Lines(selection) => {
                ensure!(!selection.source_range.is_empty(), "empty selection");
                let last = self.source_lines.last().context("patch has no source lines")?;
                ensure!(
                    *selection.source_range.end() <= last.position,
                    "selection is outside the patch"
                );
            },
        }
        Ok(())
    }

    fn includes(scope: &PatchScope, line: &SourceLine) -> bool {
        match scope {
            PatchScope::File => true,
            PatchScope::Hunk(index) => line.hunk == *index,
            PatchScope::Lines(selection) => {
                selection.source_range.contains(&line.position)
                    && match selection.side {
                        Some(SourceSide::Original) => line.original_line.is_some(),
                        Some(SourceSide::Modified) => line.modified_line.is_some(),
                        None => true,
                    }
            },
        }
    }

    pub fn selected_changed_line_count(&self, scope: &PatchScope) -> Result<usize> {
        self.validate_scope(scope)?;
        Ok(self
            .source_lines
            .iter()
            .filter(|line| line.is_changed() && Self::includes(scope, line))
            .count())
    }

    pub fn selected_patch(&self, scope: &PatchScope, direction: PatchDirection) -> Result<String> {
        self.validate_scope(scope)?;
        if matches!(scope, PatchScope::File) && direction == PatchDirection::Forward {
            return Ok(self.raw.clone());
        }
        if matches!(scope, PatchScope::File) && self.hunks.is_empty() {
            let diff = git2::Diff::from_buffer(self.raw.as_bytes())?;
            let delta = diff.get_delta(0).context("missing empty file delta")?;
            ensure!(!delta.flags().is_binary(), "binary patches cannot be reversed");
            ensure!(
                matches!(delta.status(), git2::Delta::Added | git2::Delta::Deleted),
                "metadata-only patches cannot be reversed"
            );
            let header = self
                .header
                .iter()
                .find(|line| line.starts_with("diff --git "))
                .context("missing Git header")?;
            let mode = self
                .header
                .iter()
                .find_map(|line| {
                    line.strip_prefix("new file mode ")
                        .or_else(|| line.strip_prefix("deleted file mode "))
                })
                .context("missing empty file mode")?;
            ensure!(
                matches!(mode.trim(), "100644" | "100755"),
                "only regular empty files can be reversed"
            );
            let operation = if delta.status() == git2::Delta::Added { "deleted" } else { "new" };
            let index = self
                .header
                .iter()
                .find_map(|line| line.strip_prefix("index "))
                .context("missing empty file index")?;
            let (old, new) = index.trim().split_once("..").context("invalid empty file index")?;
            let old_path = self
                .header
                .iter()
                .find_map(|line| line.strip_prefix("--- "))
                .context("missing empty original path")?
                .trim_end_matches('\n');
            let new_path = self
                .header
                .iter()
                .find_map(|line| line.strip_prefix("+++ "))
                .context("missing empty modified path")?
                .trim_end_matches('\n');
            let source = if new_path == "/dev/null" {
                "/dev/null".to_owned()
            } else {
                change_prefix(new_path, 'a')?
            };
            let target = if old_path == "/dev/null" {
                "/dev/null".to_owned()
            } else {
                change_prefix(old_path, 'b')?
            };
            let output = format!(
                "{header}{operation} file mode {mode}index {new}..{old}\n--- {source}\n+++ {target}\n"
            );
            git2::Diff::from_buffer(output.as_bytes())
                .context("invalid empty file reverse patch")?;
            return Ok(output);
        }
        if let Some(error) = &self.partial_error {
            bail!("{error}");
        }
        ensure!(
            self.selected_changed_line_count(scope)? > 0,
            "selection contains no changed lines"
        );
        let reverse = direction == PatchDirection::Reverse;
        let old_header = self
            .header
            .iter()
            .find(|line| line.starts_with("--- "))
            .context("missing original path")?;
        let new_header = self
            .header
            .iter()
            .find(|line| line.starts_with("+++ "))
            .context("missing modified path")?;
        let old_path = old_header
            .trim_end_matches('\n')
            .strip_prefix("--- ")
            .context("invalid original path")?;
        let new_path = new_header
            .trim_end_matches('\n')
            .strip_prefix("+++ ")
            .context("invalid modified path")?;
        let (source_path, target_path) =
            if reverse { (new_path, old_path) } else { (old_path, new_path) };
        let all_selected = self.selected_changed_line_count(scope)?
            == self.source_lines.iter().filter(|line| line.is_changed()).count();
        let source_missing = source_path == "/dev/null";
        let target_missing = target_path == "/dev/null" && all_selected;
        let a_path = if old_path == "/dev/null" {
            change_prefix(new_path, 'a')?
        } else {
            old_path.to_owned()
        };
        let b_path = if new_path == "/dev/null" {
            change_prefix(old_path, 'b')?
        } else {
            new_path.to_owned()
        };
        let mut output = self
            .header
            .iter()
            .find(|line| line.starts_with("diff --git "))
            .context("missing Git header")?
            .clone();
        let mode = self
            .header
            .iter()
            .find_map(|line| {
                line.strip_prefix("new file mode ")
                    .or_else(|| line.strip_prefix("deleted file mode "))
            })
            .unwrap_or("100644\n");
        if source_missing {
            output.push_str(&format!("new file mode {mode}"));
        } else if target_missing {
            output.push_str(&format!("deleted file mode {mode}"));
        }
        output.push_str(&format!(
            "--- {}\n+++ {}\n",
            if source_missing { "/dev/null" } else { &a_path },
            if target_missing { "/dev/null" } else { &b_path }
        ));
        let mut delta: i64 = 0;
        let mut remaining = self.source_lines.as_slice();
        for hunk in &self.hunks {
            let count = remaining.iter().take_while(|line| line.hunk == hunk.index).count();
            let (hunk_lines, rest) = remaining.split_at(count);
            remaining = rest;
            if !hunk_lines.iter().any(|line| line.is_changed() && Self::includes(scope, line)) {
                continue;
            }
            let mut lines: Vec<_> = hunk_lines.iter().collect();
            if reverse {
                let mut start = 0;
                while start < lines.len() {
                    if !lines[start].is_changed() {
                        start += 1;
                        continue;
                    }
                    let end = lines[start..]
                        .iter()
                        .position(|line| !line.is_changed())
                        .map_or(lines.len(), |offset| start + offset);
                    lines[start..end].sort_by_key(|line| line.origin != '+');
                    start = end;
                }
            }
            let mut body = String::new();
            let mut old_count = 0u32;
            let mut new_count = 0u32;
            let mut old_eof = false;
            let mut new_eof = false;
            for line in lines {
                let origin = match (reverse, line.origin) {
                    (true, '+') => '-',
                    (true, '-') => '+',
                    (_, origin) => origin,
                };
                let origin = match (origin, Self::includes(scope, line)) {
                    ('+', false) => continue,
                    ('-', false) => ' ',
                    (origin, _) => origin,
                };
                if origin != '+' {
                    ensure!(
                        !old_eof,
                        "selection creates content after original EOF without a newline"
                    );
                    old_count += 1;
                    old_eof = line.has_no_newline();
                }
                if origin != '-' {
                    ensure!(
                        !new_eof,
                        "selection creates content after modified EOF without a newline"
                    );
                    new_count += 1;
                    new_eof = line.has_no_newline();
                }
                body.push(origin);
                body.push_str(&line.content);
                if line.has_no_newline() {
                    body.push_str("\\ No newline at end of file\n");
                }
            }
            let (start, count) = if reverse {
                (hunk.modified_start, hunk.modified_count)
            } else {
                (hunk.original_start, hunk.original_count)
            };
            let old_offset = if count == 0 { i64::from(start) } else { i64::from(start) - 1 };
            let new_offset = old_offset + delta;
            ensure!(old_offset >= 0 && new_offset >= 0, "invalid partial hunk coordinates");
            let old_start = old_offset + i64::from(old_count > 0);
            let new_start = new_offset + i64::from(new_count > 0);
            output.push_str(&format!(
                "@@ -{old_start},{old_count} +{new_start},{new_count} @@{}\n",
                hunk.label
            ));
            output.push_str(&body);
            delta += i64::from(new_count) - i64::from(old_count);
        }
        git2::Diff::from_buffer(output.as_bytes()).context("generated partial patch is invalid")?;
        Ok(output)
    }
}

fn change_prefix(path: &str, prefix: char) -> Result<String> {
    if let Some(rest) = path.strip_prefix("a/").or_else(|| path.strip_prefix("b/")) {
        return Ok(format!("{prefix}/{rest}"));
    }
    if let Some(rest) = path.strip_prefix("\"a/").or_else(|| path.strip_prefix("\"b/")) {
        return Ok(format!("\"{prefix}/{rest}"));
    }
    bail!("unsupported Git path quoting")
}

type HunkHeader = ((u32, u32), (u32, u32), String);

fn parse_hunk_header(line: &str) -> Result<HunkHeader> {
    let rest = line.strip_prefix("@@ -").context("invalid hunk header")?;
    let (old, rest) = rest.split_once(" +").context("invalid hunk coordinates")?;
    let (new, label) = rest.split_once(" @@").context("invalid hunk label")?;
    Ok((parse_range(old)?, parse_range(new)?, label.trim_end_matches('\n').to_owned()))
}

fn parse_range(raw: &str) -> Result<(u32, u32)> {
    match raw.split_once(',') {
        Some((start, count)) => Ok((start.parse()?, count.parse()?)),
        None => Ok((raw.parse()?, 1)),
    }
}
