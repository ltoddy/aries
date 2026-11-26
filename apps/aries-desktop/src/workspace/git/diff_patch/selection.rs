use crate::workspace::git::diff_patch::error::{PatchError, Result};
use crate::workspace::git::diff_patch::parse::change_prefix;
use crate::workspace::git::diff_patch::{
    DiffPatch, LineEndpoint, PatchDirection, PatchScope, SourceLine, SourceSide,
};

impl DiffPatch {
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
