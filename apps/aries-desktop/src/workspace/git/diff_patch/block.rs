use crate::workspace::git::diff_patch::{LineSelection, PatchScope, SourceLine};

#[derive(Clone, Copy, Debug)]
pub struct ChangeBlock {
    index: usize,
    original: Option<u32>,
    modified: Option<u32>,
    range: (usize, usize),
}

impl ChangeBlock {
    pub fn new(
        index: usize,
        original: Option<u32>,
        modified: Option<u32>,
        range: (usize, usize),
    ) -> Self {
        Self { index, original, modified, range }
    }

    pub fn index(&self) -> usize {
        self.index
    }

    pub fn original(&self) -> Option<u32> {
        self.original
    }

    pub fn modified(&self) -> Option<u32> {
        self.modified
    }

    pub fn scope(&self) -> PatchScope {
        PatchScope::lines(LineSelection::new(self.range.0..=self.range.1))
    }

    pub fn contains(&self, position: usize) -> bool {
        (self.range.0..=self.range.1).contains(&position)
    }

    pub fn count(&self) -> usize {
        self.range.1 - self.range.0 + 1
    }

    pub fn is_single_sided(&self) -> bool {
        self.original.is_none() || self.modified.is_none()
    }
}

pub fn build_blocks(lines: &[SourceLine]) -> Vec<ChangeBlock> {
    let mut blocks: Vec<ChangeBlock> = Vec::new();
    let mut open = false;
    for line in lines {
        if !line.is_changed() {
            open = false;
            continue;
        }
        if !open {
            let start = line.position();
            blocks.push(ChangeBlock::new(blocks.len(), None, None, (start, start)));
            open = true;
        }
        if let Some(block) = blocks.last_mut() {
            block.original = block.original.or(line.original_line());
            block.modified = block.modified.or(line.modified_line());
            block.range.1 = line.position();
        }
    }
    blocks
}
