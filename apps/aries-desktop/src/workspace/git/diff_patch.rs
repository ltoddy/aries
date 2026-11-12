use std::ops::RangeInclusive;

use thiserror::Error;

type Result<T> = std::result::Result<T, PatchError>;

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
        if line == 0 {
            return Err(PatchError::invalid_line_number());
        }
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
        let anchor = patch
            .source_position(anchor)
            .ok_or_else(PatchError::selection_anchor_is_outside_the_patch)?;
        let head = patch
            .source_position(head)
            .ok_or_else(PatchError::selection_head_is_outside_the_patch)?;
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
    partial_error: Option<PartialPatchError>,
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
        if raw.is_empty() {
            return Err(PatchError::patch_is_empty());
        }
        let validated = git2::Diff::from_buffer(raw.as_bytes())
            .map_err(|source| PatchError::git("invalid Git patch", source))?;
        if validated.deltas().len() != 1 {
            return Err(PatchError::multiple_files());
        }
        let mut patch = Self::new(raw.to_owned());
        if let Some(delta) = validated.get_delta(0) {
            if delta.flags().is_binary() {
                patch.partial_error = Some(PartialPatchError::binary());
            }
            if !matches!(
                delta.status(),
                git2::Delta::Added
                    | git2::Delta::Deleted
                    | git2::Delta::Modified
                    | git2::Delta::Untracked
            ) {
                patch.partial_error = Some(PartialPatchError::delta());
            }
            for mode in [delta.old_file().mode(), delta.new_file().mode()] {
                if !matches!(
                    mode,
                    git2::FileMode::Unreadable
                        | git2::FileMode::Blob
                        | git2::FileMode::BlobExecutable
                ) {
                    patch.partial_error = Some(PartialPatchError::file_mode());
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
                    patch.partial_error = Some(PartialPatchError::metadata());
                }
                continue;
            }
            if line == "\\ No newline at end of file\n" || line == "\\ No newline at end of file" {
                let previous =
                    patch.source_lines.last_mut().ok_or_else(PatchError::orphan_eof_marker)?;
                if previous.no_newline {
                    return Err(PatchError::duplicate_eof_marker());
                }
                previous.no_newline = true;
                continue;
            }
            let origin = line.chars().next().ok_or_else(PatchError::empty_patch_line)?;
            if !matches!(origin, ' ' | '+' | '-') {
                return Err(PatchError::unsupported_patch_body());
            }
            if !line.ends_with('\n') {
                return Err(PatchError::patch_body_line_is_not_terminated());
            }
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
                old = old.checked_add(1).ok_or_else(PatchError::original_coordinate_overflow)?;
            }
            if origin != '-' {
                new = new.checked_add(1).ok_or_else(PatchError::modified_coordinate_overflow)?;
            }
            let hunk = patch.hunks.last_mut().ok_or_else(PatchError::missing_hunk)?;
            hunk.source_range = *hunk.source_range.start()..=position;
        }
        if let Some(hunk) = patch.hunks.last() {
            Self::validate_hunk(hunk, &patch.source_lines[hunk_start..])?;
        }
        if patch.hunks.is_empty() && patch.partial_error.is_none() {
            patch.partial_error = Some(PartialPatchError::no_text_hunks());
        }
        Ok(patch)
    }

    fn validate_hunk(hunk: &PatchHunk, lines: &[SourceLine]) -> Result<()> {
        let old = lines.iter().filter(|line| line.origin != '+').count();
        let new = lines.iter().filter(|line| line.origin != '-').count();
        if old != hunk.original_count as usize || new != hunk.modified_count as usize {
            return Err(PatchError::hunk_count_mismatch());
        }
        Ok(())
    }

    pub fn hunks(&self) -> &[PatchHunk] {
        &self.hunks
    }

    pub fn source_lines(&self) -> &[SourceLine] {
        &self.source_lines
    }

    pub fn partial_error(&self) -> Option<&str> {
        self.partial_error.as_ref().map(PartialPatchError::label)
    }

    pub fn is_discardable(&self) -> bool {
        self.partial_error.is_none()
            || (matches!(self.partial_error, Some(PartialPatchError::NoTextHunks))
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
            PatchScope::Hunk(index) => {
                if *index >= self.hunks.len() {
                    return Err(PatchError::hunk_no_longer_exists());
                }
            },
            PatchScope::Lines(selection) => {
                if selection.source_range.is_empty() {
                    return Err(PatchError::empty_selection());
                }
                let last =
                    self.source_lines.last().ok_or_else(PatchError::patch_has_no_source_lines)?;
                if *selection.source_range.end() > last.position {
                    return Err(PatchError::selection_is_outside_the_patch());
                }
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
            let diff = git2::Diff::from_buffer(self.raw.as_bytes())
                .map_err(|source| PatchError::git("reading reverse patch", source))?;
            let delta = diff.get_delta(0).ok_or_else(PatchError::missing_empty_file_delta)?;
            if delta.flags().is_binary() {
                return Err(PatchError::binary_reverse_unsupported());
            }
            if !matches!(delta.status(), git2::Delta::Added | git2::Delta::Deleted) {
                return Err(PatchError::metadata_reverse_unsupported());
            }
            let header = self
                .header
                .iter()
                .find(|line| line.starts_with("diff --git "))
                .ok_or_else(PatchError::missing_git_header)?;
            let mode = self
                .header
                .iter()
                .find_map(|line| {
                    line.strip_prefix("new file mode ")
                        .or_else(|| line.strip_prefix("deleted file mode "))
                })
                .ok_or_else(PatchError::missing_empty_file_mode)?;
            if !matches!(mode.trim(), "100644" | "100755") {
                return Err(PatchError::empty_file_mode_unsupported());
            }
            let operation = if delta.status() == git2::Delta::Added { "deleted" } else { "new" };
            let index = self
                .header
                .iter()
                .find_map(|line| line.strip_prefix("index "))
                .ok_or_else(PatchError::missing_empty_file_index)?;
            let (old, new) =
                index.trim().split_once("..").ok_or_else(PatchError::invalid_empty_file_index)?;
            let old_path = self
                .header
                .iter()
                .find_map(|line| line.strip_prefix("--- "))
                .ok_or_else(PatchError::missing_empty_original_path)?
                .trim_end_matches('\n');
            let new_path = self
                .header
                .iter()
                .find_map(|line| line.strip_prefix("+++ "))
                .ok_or_else(PatchError::missing_empty_modified_path)?
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
                .map_err(|source| PatchError::git("invalid empty file reverse patch", source))?;
            return Ok(output);
        }
        if let Some(error) = &self.partial_error {
            return Err(PatchError::partial(error.clone()));
        }
        if self.selected_changed_line_count(scope)? == 0 {
            return Err(PatchError::no_changed_lines());
        }
        let reverse = direction == PatchDirection::Reverse;
        let old_header = self
            .header
            .iter()
            .find(|line| line.starts_with("--- "))
            .ok_or_else(PatchError::missing_original_path)?;
        let new_header = self
            .header
            .iter()
            .find(|line| line.starts_with("+++ "))
            .ok_or_else(PatchError::missing_modified_path)?;
        let old_path = old_header
            .trim_end_matches('\n')
            .strip_prefix("--- ")
            .ok_or_else(PatchError::invalid_original_path)?;
        let new_path = new_header
            .trim_end_matches('\n')
            .strip_prefix("+++ ")
            .ok_or_else(PatchError::invalid_modified_path)?;
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
            .ok_or_else(PatchError::missing_git_header)?
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
                    if old_eof {
                        return Err(PatchError::original_eof_conflict());
                    }
                    old_count += 1;
                    old_eof = line.has_no_newline();
                }
                if origin != '-' {
                    if new_eof {
                        return Err(PatchError::modified_eof_conflict());
                    }
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
            if old_offset < 0 || new_offset < 0 {
                return Err(PatchError::invalid_partial_hunk_coordinates());
            }
            let old_start = old_offset + i64::from(old_count > 0);
            let new_start = new_offset + i64::from(new_count > 0);
            output.push_str(&format!(
                "@@ -{old_start},{old_count} +{new_start},{new_count} @@{}\n",
                hunk.label
            ));
            output.push_str(&body);
            delta += i64::from(new_count) - i64::from(old_count);
        }
        git2::Diff::from_buffer(output.as_bytes())
            .map_err(|source| PatchError::git("generated partial patch is invalid", source))?;
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
    Err(PatchError::unsupported_path_quoting())
}

type HunkHeader = ((u32, u32), (u32, u32), String);

fn parse_hunk_header(line: &str) -> Result<HunkHeader> {
    let rest = line.strip_prefix("@@ -").ok_or_else(PatchError::invalid_hunk_header)?;
    let (old, rest) = rest.split_once(" +").ok_or_else(PatchError::invalid_hunk_coordinates)?;
    let (new, label) = rest.split_once(" @@").ok_or_else(PatchError::invalid_hunk_label)?;
    Ok((parse_range(old)?, parse_range(new)?, label.trim_end_matches('\n').to_owned()))
}

fn parse_range(raw: &str) -> Result<(u32, u32)> {
    match raw.split_once(',') {
        Some((start, count)) => Ok((
            start.parse().map_err(|source| PatchError::coordinate(start, source))?,
            count.parse().map_err(|source| PatchError::coordinate(count, source))?,
        )),
        None => Ok((raw.parse().map_err(|source| PatchError::coordinate(raw, source))?, 1)),
    }
}

#[derive(Debug, Error)]
pub enum PatchError {
    #[error("line numbers are one-based")]
    InvalidLineNumber,
    #[error("patch is empty")]
    PatchIsEmpty,
    #[error("expected exactly one file patch")]
    MultipleFiles,
    #[error("duplicate EOF marker")]
    DuplicateEofMarker,
    #[error("unsupported patch body")]
    UnsupportedPatchBody,
    #[error("patch body line is not terminated")]
    PatchBodyLineIsNotTerminated,
    #[error("hunk counts do not match body")]
    HunkCountMismatch,
    #[error("hunk no longer exists")]
    HunkNoLongerExists,
    #[error("empty selection")]
    EmptySelection,
    #[error("selection is outside the patch")]
    SelectionIsOutsideThePatch,
    #[error("binary patches cannot be reversed")]
    BinaryReverseUnsupported,
    #[error("metadata-only patches cannot be reversed")]
    MetadataReverseUnsupported,
    #[error("only regular empty files can be reversed")]
    EmptyFileModeUnsupported,
    #[error("selection contains no changed lines")]
    NoChangedLines,
    #[error("selection creates content after original EOF without a newline")]
    OriginalEofConflict,
    #[error("selection creates content after modified EOF without a newline")]
    ModifiedEofConflict,
    #[error("invalid partial hunk coordinates")]
    InvalidPartialHunkCoordinates,
    #[error("unsupported Git path quoting")]
    UnsupportedPathQuoting,
    #[error("selection anchor is outside the patch")]
    SelectionAnchorIsOutsideThePatch,
    #[error("selection head is outside the patch")]
    SelectionHeadIsOutsideThePatch,
    #[error("orphan EOF marker")]
    OrphanEofMarker,
    #[error("empty patch line")]
    EmptyPatchLine,
    #[error("original coordinate overflow")]
    OriginalCoordinateOverflow,
    #[error("modified coordinate overflow")]
    ModifiedCoordinateOverflow,
    #[error("missing hunk")]
    MissingHunk,
    #[error("patch has no source lines")]
    PatchHasNoSourceLines,
    #[error("missing empty file delta")]
    MissingEmptyFileDelta,
    #[error("missing Git header")]
    MissingGitHeader,
    #[error("missing empty file mode")]
    MissingEmptyFileMode,
    #[error("missing empty file index")]
    MissingEmptyFileIndex,
    #[error("invalid empty file index")]
    InvalidEmptyFileIndex,
    #[error("missing empty original path")]
    MissingEmptyOriginalPath,
    #[error("missing empty modified path")]
    MissingEmptyModifiedPath,
    #[error("missing original path")]
    MissingOriginalPath,
    #[error("missing modified path")]
    MissingModifiedPath,
    #[error("invalid original path")]
    InvalidOriginalPath,
    #[error("invalid modified path")]
    InvalidModifiedPath,
    #[error("invalid hunk header")]
    InvalidHunkHeader,
    #[error("invalid hunk coordinates")]
    InvalidHunkCoordinates,
    #[error("invalid hunk label")]
    InvalidHunkLabel,
    #[error("{operation}")]
    Git { operation: &'static str, source: git2::Error },
    #[error("invalid patch coordinate {value}")]
    Coordinate { value: String, source: std::num::ParseIntError },
    #[error(transparent)]
    Partial { source: PartialPatchError },
}

impl PatchError {
    pub fn invalid_line_number() -> Self {
        Self::InvalidLineNumber
    }
    pub fn patch_is_empty() -> Self {
        Self::PatchIsEmpty
    }
    pub fn multiple_files() -> Self {
        Self::MultipleFiles
    }
    pub fn duplicate_eof_marker() -> Self {
        Self::DuplicateEofMarker
    }
    pub fn unsupported_patch_body() -> Self {
        Self::UnsupportedPatchBody
    }
    pub fn patch_body_line_is_not_terminated() -> Self {
        Self::PatchBodyLineIsNotTerminated
    }
    pub fn hunk_count_mismatch() -> Self {
        Self::HunkCountMismatch
    }
    pub fn hunk_no_longer_exists() -> Self {
        Self::HunkNoLongerExists
    }
    pub fn empty_selection() -> Self {
        Self::EmptySelection
    }
    pub fn selection_is_outside_the_patch() -> Self {
        Self::SelectionIsOutsideThePatch
    }
    pub fn binary_reverse_unsupported() -> Self {
        Self::BinaryReverseUnsupported
    }
    pub fn metadata_reverse_unsupported() -> Self {
        Self::MetadataReverseUnsupported
    }
    pub fn empty_file_mode_unsupported() -> Self {
        Self::EmptyFileModeUnsupported
    }
    pub fn no_changed_lines() -> Self {
        Self::NoChangedLines
    }
    pub fn original_eof_conflict() -> Self {
        Self::OriginalEofConflict
    }
    pub fn modified_eof_conflict() -> Self {
        Self::ModifiedEofConflict
    }
    pub fn invalid_partial_hunk_coordinates() -> Self {
        Self::InvalidPartialHunkCoordinates
    }
    pub fn unsupported_path_quoting() -> Self {
        Self::UnsupportedPathQuoting
    }
    pub fn selection_anchor_is_outside_the_patch() -> Self {
        Self::SelectionAnchorIsOutsideThePatch
    }
    pub fn selection_head_is_outside_the_patch() -> Self {
        Self::SelectionHeadIsOutsideThePatch
    }
    pub fn orphan_eof_marker() -> Self {
        Self::OrphanEofMarker
    }
    pub fn empty_patch_line() -> Self {
        Self::EmptyPatchLine
    }
    pub fn original_coordinate_overflow() -> Self {
        Self::OriginalCoordinateOverflow
    }
    pub fn modified_coordinate_overflow() -> Self {
        Self::ModifiedCoordinateOverflow
    }
    pub fn missing_hunk() -> Self {
        Self::MissingHunk
    }
    pub fn patch_has_no_source_lines() -> Self {
        Self::PatchHasNoSourceLines
    }
    pub fn missing_empty_file_delta() -> Self {
        Self::MissingEmptyFileDelta
    }
    pub fn missing_git_header() -> Self {
        Self::MissingGitHeader
    }
    pub fn missing_empty_file_mode() -> Self {
        Self::MissingEmptyFileMode
    }
    pub fn missing_empty_file_index() -> Self {
        Self::MissingEmptyFileIndex
    }
    pub fn invalid_empty_file_index() -> Self {
        Self::InvalidEmptyFileIndex
    }
    pub fn missing_empty_original_path() -> Self {
        Self::MissingEmptyOriginalPath
    }
    pub fn missing_empty_modified_path() -> Self {
        Self::MissingEmptyModifiedPath
    }
    pub fn missing_original_path() -> Self {
        Self::MissingOriginalPath
    }
    pub fn missing_modified_path() -> Self {
        Self::MissingModifiedPath
    }
    pub fn invalid_original_path() -> Self {
        Self::InvalidOriginalPath
    }
    pub fn invalid_modified_path() -> Self {
        Self::InvalidModifiedPath
    }
    pub fn invalid_hunk_header() -> Self {
        Self::InvalidHunkHeader
    }
    pub fn invalid_hunk_coordinates() -> Self {
        Self::InvalidHunkCoordinates
    }
    pub fn invalid_hunk_label() -> Self {
        Self::InvalidHunkLabel
    }
    pub fn git(operation: &'static str, source: git2::Error) -> Self {
        Self::Git { operation, source }
    }
    pub fn coordinate(value: &str, source: std::num::ParseIntError) -> Self {
        Self::Coordinate { value: value.to_owned(), source }
    }
    pub fn partial(source: PartialPatchError) -> Self {
        Self::Partial { source }
    }
}

#[derive(Clone, Debug, Error)]
pub enum PartialPatchError {
    #[error("binary patches do not support partial operations")]
    Binary,
    #[error("rename and type changes do not support partial operations")]
    Delta,
    #[error("symlinks and submodules do not support partial operations")]
    FileMode,
    #[error("metadata and binary changes do not support partial operations")]
    Metadata,
    #[error("patch has no text hunks")]
    NoTextHunks,
}
impl PartialPatchError {
    fn binary() -> Self {
        Self::Binary
    }
    fn delta() -> Self {
        Self::Delta
    }
    fn file_mode() -> Self {
        Self::FileMode
    }
    fn metadata() -> Self {
        Self::Metadata
    }
    fn no_text_hunks() -> Self {
        Self::NoTextHunks
    }
    fn label(&self) -> &'static str {
        match self {
            Self::Binary => "binary patches do not support partial operations",
            Self::Delta => "rename and type changes do not support partial operations",
            Self::FileMode => "symlinks and submodules do not support partial operations",
            Self::Metadata => "metadata and binary changes do not support partial operations",
            Self::NoTextHunks => "patch has no text hunks",
        }
    }
}
