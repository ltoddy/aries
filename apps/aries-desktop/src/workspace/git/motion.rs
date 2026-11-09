use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use gpui_kit::UniformListScrollHandle;
use gpui_kit::base::motion::Easing;

use crate::workspace::git::repository::{ChangeGroup, ChangedFile, FileChange, GitInfo};

#[derive(Clone)]
pub struct Fade {
    from: f32,
    to: f32,
    started: Instant,
    duration: Duration,
    easing: Easing,
}

impl Fade {
    pub fn settled(value: f32) -> Self {
        Self::new(value, value, Instant::now(), Duration::ZERO, Easing::Linear)
    }

    pub fn new(from: f32, to: f32, now: Instant, duration: Duration, easing: Easing) -> Self {
        Self { from, to, started: now, duration, easing }
    }

    pub fn sample(&self, now: Instant) -> f32 {
        if !self.active(now) {
            return self.to.clamp(0.0, 1.0);
        }
        let progress =
            now.saturating_duration_since(self.started).as_secs_f32() / self.duration.as_secs_f32();
        (self.from + (self.to - self.from) * self.easing.sample(progress)).clamp(0.0, 1.0)
    }

    pub fn active(&self, now: Instant) -> bool {
        now.saturating_duration_since(self.started) < self.duration
    }

    pub fn retarget(&mut self, to: f32, now: Instant, duration: Duration, easing: Easing) {
        *self = Self::new(self.sample(now), to, now, duration, easing);
    }

    pub fn settle(&mut self) {
        self.from = self.to;
        self.duration = Duration::ZERO;
    }
}

impl Default for Fade {
    fn default() -> Self {
        Self::settled(1.0)
    }
}

#[derive(Clone)]
pub struct GitRow {
    group: ChangeGroup,
    file: FileChange,
    visibility: Fade,
    status: Fade,
    lines: Fade,
    exiting: bool,
}

impl GitRow {
    pub fn file(&self) -> &FileChange {
        &self.file
    }
    pub fn visibility(&self) -> &Fade {
        &self.visibility
    }
    pub fn status(&self) -> &Fade {
        &self.status
    }
    pub fn lines(&self) -> &Fade {
        &self.lines
    }
    pub fn is_exiting(&self) -> bool {
        self.exiting
    }
}

pub struct GitMotion {
    rows: Vec<GitRow>,
    summary: Fade,
    previous_summary: Option<String>,
    scroll: [UniformListScrollHandle; 2],
    panel: Fade,
    cwd: Option<PathBuf>,
    snapshot: Option<GitInfo>,
    initialized: bool,
}

impl Default for GitMotion {
    fn default() -> Self {
        Self {
            rows: Vec::new(),
            summary: Fade::default(),
            previous_summary: None,
            scroll: std::array::from_fn(|_| UniformListScrollHandle::new()),
            panel: Fade::settled(0.0),
            cwd: None,
            snapshot: None,
            initialized: false,
        }
    }
}

impl GitMotion {
    pub fn all_rows(&self) -> &[GitRow] {
        &self.rows
    }
    pub fn summary(&self) -> &Fade {
        &self.summary
    }
    pub fn previous_summary(&self) -> Option<&String> {
        self.previous_summary.as_ref()
    }
    pub fn scroll(&self, group: ChangeGroup) -> &UniformListScrollHandle {
        &self.scroll[match group {
            ChangeGroup::Staged => 0,
            ChangeGroup::Unstaged => 1,
        }]
    }
    pub fn panel(&self) -> &Fade {
        &self.panel
    }
    pub fn panel_mut(&mut self) -> &mut Fade {
        &mut self.panel
    }

    pub fn reset(&mut self, cwd: &Path) -> bool {
        if self.cwd.as_deref() == Some(cwd) {
            return false;
        }
        *self = Self { cwd: Some(cwd.to_owned()), ..Self::default() };
        true
    }

    pub fn sync(
        &mut self,
        info: Option<&GitInfo>,
        now: Instant,
        duration: Duration,
        easing: Easing,
        animate: bool,
    ) -> bool {
        let settled = if animate { false } else { self.cleanup(now, true) };
        if self.snapshot.as_ref() == info {
            return settled;
        }
        let Some(files) = info.and_then(GitInfo::changed_files) else {
            self.rows.clear();
            self.summary.settle();
            self.previous_summary = None;
            self.initialized = false;
            self.snapshot = info.cloned();
            return true;
        };
        let animate = animate && self.initialized;
        let short = duration;
        if animate && self.snapshot.as_ref().map(GitInfo::summary) != info.map(GitInfo::summary) {
            let from = if self.summary.active(now)
                && self.previous_summary.as_deref() == info.map(GitInfo::summary).as_deref()
            {
                1. - self.summary.sample(now)
            } else {
                0.
            };
            self.previous_summary = self.snapshot.as_ref().map(GitInfo::summary);
            self.summary = Fade::new(from, 1.0, now, short, easing.clone());
        }
        self.cleanup(now, false);
        let changes = [ChangeGroup::Staged, ChangeGroup::Unstaged].map(|group| {
            let mut changes = HashMap::with_capacity(files.len());
            for file in files.iter().filter_map(|file| file.change(group)) {
                changes.entry(file.path()).or_insert(file);
            }
            changes
        });
        for row in &mut self.rows {
            let group_ix = match row.group {
                ChangeGroup::Staged => 0,
                ChangeGroup::Unstaged => 1,
            };
            let next = changes[group_ix].get(row.file.path()).copied();
            if let Some(file) = next {
                if row.exiting {
                    row.exiting = false;
                    row.visibility.retarget(1.0, now, duration, easing.clone());
                }
                if animate {
                    if ChangedFile::status_kind(row.file.status())
                        != ChangedFile::status_kind(file.status())
                    {
                        row.status = Fade::new(
                            if row.status.active(now) { row.status.sample(now) } else { 0. },
                            1.0,
                            now,
                            short,
                            easing.clone(),
                        );
                    }
                    if row.file.lines() != file.lines() {
                        row.lines = Fade::new(
                            if row.lines.active(now) { row.lines.sample(now) } else { 0. },
                            1.0,
                            now,
                            short,
                            easing.clone(),
                        );
                    }
                }
                row.file = file.clone();
            } else if !row.exiting {
                row.exiting = true;
                row.visibility.retarget(0.0, now, duration, easing.clone());
            }
        }
        let mut previous: Vec<_> = std::mem::take(&mut self.rows).into_iter().map(Some).collect();
        for group in [ChangeGroup::Staged, ChangeGroup::Unstaged] {
            let exits: Vec<_> = previous
                .iter_mut()
                .filter(|row| row.as_ref().is_some_and(|row| row.group == group))
                .enumerate()
                .filter_map(|(slot, row)| {
                    if row.as_ref().unwrap().exiting {
                        Some((slot, row.take().unwrap()))
                    } else {
                        None
                    }
                })
                .collect();
            let mut previous_indices: HashMap<PathBuf, Vec<usize>> = HashMap::new();
            for (ix, row) in previous.iter().enumerate().rev() {
                if let Some(row) = row.as_ref().filter(|row| row.group == group) {
                    previous_indices.entry(row.file.path().to_owned()).or_default().push(ix);
                }
            }
            let mut rows = Vec::new();
            for file in files.iter().filter_map(|file| file.change(group)) {
                let existing = previous_indices
                    .get_mut(file.path())
                    .and_then(Vec::pop)
                    .and_then(|ix| previous[ix].take());
                rows.push(existing.unwrap_or_else(|| GitRow {
                    group,
                    file: file.clone(),
                    visibility: if animate {
                        Fade::new(0.0, 1.0, now, duration, easing.clone())
                    } else {
                        Fade::default()
                    },
                    status: Fade::default(),
                    lines: Fade::default(),
                    exiting: false,
                }));
            }
            for (slot, row) in exits {
                rows.insert(slot.min(rows.len()), row);
            }
            self.rows.extend(rows);
        }
        if !animate {
            self.cleanup(now, true);
        }
        self.snapshot = info.cloned();
        self.initialized = true;
        true
    }

    pub fn cleanup(&mut self, now: Instant, reduced: bool) -> bool {
        if reduced || !self.summary.active(now) {
            self.previous_summary = None;
        }
        let before = self.rows.len();
        self.rows.retain(|row| !row.exiting || (!reduced && row.visibility.active(now)));
        let mut changed = before != self.rows.len();
        for fade in
            std::iter::once(&mut self.summary).chain(std::iter::once(&mut self.panel)).chain(
                self.rows
                    .iter_mut()
                    .flat_map(|row| [&mut row.visibility, &mut row.status, &mut row.lines]),
            )
        {
            if reduced || !fade.active(now) {
                changed |= !fade.duration.is_zero();
                fade.settle();
            }
        }
        changed
    }

    pub fn rows(&self, group: ChangeGroup) -> impl Iterator<Item = &GitRow> {
        self.rows.iter().filter(move |row| row.group == group)
    }
}

pub fn accept_result(cwd: &Path, revision: u64, result_cwd: &Path, result_revision: u64) -> bool {
    cwd == result_cwd && revision == result_revision
}
