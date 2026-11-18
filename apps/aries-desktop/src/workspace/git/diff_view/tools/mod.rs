use std::sync::Arc;

use thiserror::Error;

use crate::error::{details, log, report};
use crate::workspace::git::diff_operations::GitOperationError;
use crate::workspace::git::diff_patch::PatchError;
use crate::workspace::git::error::RepositoryError;

type Result<T> = std::result::Result<T, ToolError>;
use git2::Repository;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariant, ButtonVariants};
use gpui_kit::component::diff::{DiffDecoration, DiffLinePosition, DiffLineRange, DiffSide};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::menu::{DropdownMenu, PopupMenu, PopupMenuItem};
use gpui_kit::component::{ActiveTheme, Disableable, Sizable, WindowExt};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::workspace::git::diff_operations::{DiffAction, DiffRequest, DiscardBackup};
use crate::workspace::git::diff_patch::{LineEndpoint, LineSelection, PatchScope, SourceSide};
use crate::workspace::git::diff_view::GitDiffView;
use crate::workspace::git::repository::ChangeGroup;

mod render;
mod search;
mod selection;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ToolScope {
    File,
    Hunk,
    Lines,
}

impl ToolScope {
    pub fn file() -> Self {
        Self::File
    }
    fn hunk() -> Self {
        Self::Hunk
    }
    fn lines() -> Self {
        Self::Lines
    }
    fn label(self) -> &'static str {
        match self {
            Self::File => "整个文件",
            Self::Hunk => "当前块",
            Self::Lines => "选中行",
        }
    }
}

#[derive(Clone)]
pub struct ToolSnapshot {
    revision: u64,
    patch: Arc<String>,
    range: Option<DiffLineRange>,
    hunk: Option<usize>,
    location: Option<DiffLinePosition>,
    available: [bool; 3],
}

impl ToolSnapshot {
    fn new(view: &GitDiffView) -> Self {
        Self {
            revision: view.revision,
            patch: view.patch.clone(),
            range: view.selection_snapshot.clone(),
            hunk: view.current_hunk,
            location: view.location.clone(),
            available: [
                view.scope_available(ToolScope::file()),
                view.scope_available(ToolScope::hunk()),
                view.scope_available(ToolScope::lines()),
            ],
        }
    }

    fn scope_available(&self, scope: ToolScope) -> bool {
        self.available[match scope {
            ToolScope::File => 0,
            ToolScope::Hunk => 1,
            ToolScope::Lines => 2,
        }]
    }

    fn is_current(&self, view: &GitDiffView) -> bool {
        self.revision == view.revision && Arc::ptr_eq(&self.patch, &view.patch)
    }
}

impl GitDiffView {
    pub fn tool_snapshot(&self) -> ToolSnapshot {
        ToolSnapshot::new(self)
    }

    pub fn restore_tool_snapshot(
        &mut self,
        snapshot: &ToolSnapshot,
        cx: &mut Context<Self>,
    ) -> bool {
        if !snapshot.is_current(self) {
            return false;
        }
        self.state.update(cx, |state, cx| {
            state.set_selected_lines(None, cx);
            state.set_selected_lines(snapshot.range.clone(), cx);
        });
        self.selection_changed(cx);
        self.current_hunk = snapshot.hunk;
        self.location = snapshot.location.clone();
        cx.notify();
        true
    }

    pub fn reset_tools(&mut self) {
        self.model = None;
        self.tool_error = None;
        self.selected_scope = None;
        self.selection_snapshot = None;
        self.selected_count = 0;
        self.current_hunk = None;
        self.location = None;
        self.search_open = false;
        self.search_revision = self.search_revision.wrapping_add(1);
        self.search_task = None;
        self.search_loading = false;
        self.matches.clear();
        self.match_index = None;
        self.decorations.clear();
        self.backups.clear();
        self.backup_task = None;
    }

    pub fn load_backups(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.is_open() {
            return;
        }
        let directory = self.directory.clone();
        let revision = self.revision;
        self.backup_task = Some(cx.spawn_in(window, async move |view, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let repository = Repository::discover(&directory).map_err(|source| {
                        RepositoryError::git("discovering backup repository", &directory, source)
                    })?;
                    let root = std::fs::canonicalize(
                        repository.workdir().ok_or_else(GitOperationError::no_workdir)?,
                    )
                    .map_err(GitOperationError::resolving_repository_workdir)?;
                    let mut backups = Vec::new();
                    let mut errors = Vec::new();
                    for entry in DiscardBackup::entries()? {
                        match entry {
                            Ok(backup) if backup.root() == root => backups.push(backup),
                            Ok(_) => {},
                            Err(error) => errors.push(error),
                        }
                    }
                    Ok::<_, GitOperationError>((backups, errors))
                })
                .await;
            let _ = view.update_in(cx, |this, window, cx| {
                if this.revision != revision {
                    return;
                }
                match result {
                    Ok((backups, errors)) => {
                        this.backups = backups;
                        if !errors.is_empty() {
                            this.tool_error = Some(format!(
                                "部分备份无法读取：{}",
                                errors
                                    .iter()
                                    .map(|error| details(error))
                                    .collect::<Vec<_>>()
                                    .join("；")
                            ));
                            if let Some(error) = errors.first() {
                                report(
                                    format!("{} 条备份无法读取，请检查备份文件", errors.len()),
                                    error,
                                    window,
                                    cx,
                                );
                            }
                            for error in errors.iter().skip(1) {
                                log("部分备份无法读取", error);
                            }
                        }
                    },
                    Err(error) => {
                        this.tool_error = Some(format!("无法加载丢弃备份：{}", details(&error)));
                        report("无法加载丢弃备份", &error, window, cx);
                    },
                }
                cx.notify();
            });
        }));
    }
}

fn endpoint(side: DiffSide, line: usize) -> Result<LineEndpoint> {
    Ok(LineEndpoint::new(
        match side {
            DiffSide::Original => SourceSide::original(),
            DiffSide::Modified => SourceSide::modified(),
        },
        u32::try_from(line).map_err(|source| ToolError::line_number(line, source))?,
    )?)
}

fn dispatch_change(
    owner: WeakEntity<crate::workspace::SessionView>,
    request: DiffRequest,
    window: &mut Window,
    cx: &mut App,
) {
    window.defer(cx, move |window, cx| {
        let _ = owner.update(cx, |session, cx| {
            session.with_git_panel(|panel| panel.change_diff(request, window, cx))
        });
    });
}

#[derive(Debug, Error)]
enum ToolError {
    #[error(transparent)]
    Patch { source: PatchError },
    #[error("line number {line} exceeds supported patch coordinates")]
    LineNumber { line: usize, source: std::num::TryFromIntError },
}

impl ToolError {
    fn line_number(line: usize, source: std::num::TryFromIntError) -> Self {
        Self::LineNumber { line, source }
    }
}

impl From<PatchError> for ToolError {
    fn from(source: PatchError) -> Self {
        Self::Patch { source }
    }
}
