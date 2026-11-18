use crate::workspace::git::diff_patch::error::{PartialPatchError, PatchError, Result};
use crate::workspace::git::diff_patch::{DiffPatch, PatchHunk, SourceLine};

impl DiffPatch {
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
}

pub fn change_prefix(path: &str, prefix: char) -> Result<String> {
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
