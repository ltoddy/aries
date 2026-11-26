use std::ops::RangeInclusive;

mod block;
mod error;
mod parse;
mod selection;

pub use self::block::ChangeBlock;
use self::error::Result;
pub use self::error::{PartialPatchError, PatchError};

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
    Lines(LineSelection),
}

impl PatchScope {
    pub fn file() -> Self {
        Self::File
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
}

#[derive(Clone, Debug)]
pub struct DiffPatch {
    raw: String,
    header: Vec<String>,
    hunks: Vec<PatchHunk>,
    source_lines: Vec<SourceLine>,
    blocks: Vec<ChangeBlock>,
    partial_error: Option<PartialPatchError>,
}

impl DiffPatch {
    fn new(raw: String) -> Self {
        Self {
            raw,
            header: Vec::new(),
            hunks: Vec::new(),
            source_lines: Vec::new(),
            blocks: Vec::new(),
            partial_error: None,
        }
    }

    pub fn source_lines(&self) -> &[SourceLine] {
        &self.source_lines
    }

    pub fn blocks(&self) -> &[ChangeBlock] {
        &self.blocks
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
}
