use serde::{Deserialize, Serialize};
use similar::{DiffOp, TextDiff};

pub fn diff(original: &str, updated: &str) -> (Vec<Hunk>, usize, usize) {
    let diff = TextDiff::from_lines(original, updated);

    let mut additions = 0_usize;
    let mut deletions = 0_usize;
    let mut hunks = Vec::<Hunk>::new();
    for op in diff.ops() {
        match op.tag() {
            similar::DiffTag::Equal => continue,
            similar::DiffTag::Insert => additions += op.new_range().len(),
            similar::DiffTag::Delete => deletions += op.old_range().len(),
            similar::DiffTag::Replace => {
                deletions += op.old_range().len();
                additions += op.new_range().len();
            },
        }
        hunks.push(Hunk::from_diff(op, &diff));
    }

    (hunks, additions, deletions)
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct Hunk {
    pub old_start: usize,
    pub old_lines: usize,
    pub new_start: usize,
    pub new_lines: usize,
    // 带有 patch 的前缀
    pub lines: Vec<String>,
}

impl Hunk {
    pub fn new(
        old_start: usize,
        old_lines: usize,
        new_start: usize,
        new_lines: usize,
        lines: Vec<String>,
    ) -> Self {
        Self { old_start, old_lines, new_start, new_lines, lines }
    }

    pub fn from_diff(op: &DiffOp, diff: &TextDiff<str>) -> Self {
        let old_start = op.old_range().start;
        let old_lines = op.old_range().len();
        let new_start = op.new_range().start;
        let new_lines = op.new_range().len();

        let mut lines = Vec::new();
        for idx in op.old_range() {
            if let Some(line) = diff.old_slice(idx) {
                lines.push(format!("-{}", line.trim_end_matches('\n')));
            }
        }
        for idx in op.new_range() {
            if let Some(line) = diff.new_slice(idx) {
                lines.push(format!("+{}", line.trim_end_matches('\n')));
            }
        }

        Self::new(old_start, old_lines, new_start, new_lines, lines)
    }
}
