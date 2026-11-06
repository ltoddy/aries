use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use gpui_kit::UniformListScrollHandle;
use gpui_kit::base::motion::Easing;

use crate::chat::view::git_info::{ChangeGroup, ChangedFile, FileChange, GitInfo};

#[derive(Clone)]
pub(crate) struct Fade {
    from: f32,
    to: f32,
    started: Instant,
    duration: Duration,
    easing: Easing,
}

impl Fade {
    pub(crate) fn settled(value: f32) -> Self {
        Self::new(value, value, Instant::now(), Duration::ZERO, Easing::Linear)
    }

    pub(crate) fn new(
        from: f32,
        to: f32,
        now: Instant,
        duration: Duration,
        easing: Easing,
    ) -> Self {
        Self { from, to, started: now, duration, easing }
    }

    pub(crate) fn sample(&self, now: Instant) -> f32 {
        if !self.active(now) {
            return self.to.clamp(0.0, 1.0);
        }
        let progress =
            now.saturating_duration_since(self.started).as_secs_f32() / self.duration.as_secs_f32();
        (self.from + (self.to - self.from) * self.easing.sample(progress)).clamp(0.0, 1.0)
    }

    pub(crate) fn active(&self, now: Instant) -> bool {
        now.saturating_duration_since(self.started) < self.duration
    }

    pub(crate) fn retarget(&mut self, to: f32, now: Instant, duration: Duration, easing: Easing) {
        *self = Self::new(self.sample(now), to, now, duration, easing);
    }

    pub(crate) fn settle(&mut self) {
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
pub(crate) struct GitRow {
    pub(crate) group: ChangeGroup,
    pub(crate) file: FileChange,
    pub(crate) visibility: Fade,
    pub(crate) status: Fade,
    pub(crate) lines: Fade,
    pub(crate) exiting: bool,
}

pub(crate) struct GitMotion {
    pub(crate) rows: Vec<GitRow>,
    pub(crate) summary: Fade,
    pub(crate) previous_summary: Option<String>,
    pub(crate) scroll: [UniformListScrollHandle; 2],
    pub(crate) panel: Fade,
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
    pub(crate) fn reset(&mut self, cwd: &Path) -> bool {
        if self.cwd.as_deref() == Some(cwd) {
            return false;
        }
        *self = Self { cwd: Some(cwd.to_owned()), ..Self::default() };
        true
    }

    pub(crate) fn sync(
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
        for row in &mut self.rows {
            let next = files
                .iter()
                .filter_map(|file| file.change(row.group))
                .find(|file| file.path == row.file.path);
            if let Some(file) = next {
                if row.exiting {
                    row.exiting = false;
                    row.visibility.retarget(1.0, now, duration, easing.clone());
                }
                if animate {
                    if ChangedFile::status_kind(row.file.status)
                        != ChangedFile::status_kind(file.status)
                    {
                        row.status = Fade::new(
                            if row.status.active(now) { row.status.sample(now) } else { 0. },
                            1.0,
                            now,
                            short,
                            easing.clone(),
                        );
                    }
                    if row.file.lines != file.lines {
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
            let mut rows = Vec::new();
            for file in files.iter().filter_map(|file| file.change(group)) {
                let existing = previous.iter_mut().find(|row| {
                    row.as_ref().is_some_and(|row| row.group == group && row.file.path == file.path)
                });
                rows.push(existing.and_then(Option::take).unwrap_or_else(|| GitRow {
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

    pub(crate) fn cleanup(&mut self, now: Instant, reduced: bool) -> bool {
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

    pub(crate) fn rows(&self, group: ChangeGroup) -> impl Iterator<Item = &GitRow> {
        self.rows.iter().filter(move |row| row.group == group)
    }
}

pub(crate) fn accept_result(
    cwd: &Path,
    revision: u64,
    result_cwd: &Path,
    result_revision: u64,
) -> bool {
    cwd == result_cwd && revision == result_revision
}
