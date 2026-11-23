use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::clipboard::Clipboard;
use gpui_kit::component::diff::{
    Diff, DiffDecoration, DiffFile, DiffFileStatus, DiffLinePosition, DiffLineRange, DiffMode,
    DiffSide, DiffState,
};
use gpui_kit::component::input::InputState;
use gpui_kit::component::menu::{ContextMenuExt, DropdownMenu, PopupMenuItem};
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{ActiveTheme, Disableable, Icon, Selectable, Sizable};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::error::details;
use crate::workspace::SessionView;
use crate::workspace::files::OpenFile;
use crate::workspace::git::diff_operations::DiscardBackupEntry;
use crate::workspace::git::diff_patch::{DiffPatch, PatchScope};
use crate::workspace::git::error::DiffLoadError;
use crate::workspace::git::repository::{ChangeGroup, GitInfo, read_file_diff};

mod tools;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiffTarget {
    path: PathBuf,
    group: ChangeGroup,
}

impl DiffTarget {
    fn new(path: PathBuf, group: ChangeGroup) -> Self {
        Self { path, group }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn group(&self) -> ChangeGroup {
        self.group
    }
}

pub struct StageNavigation {
    target: DiffTarget,
    revision: u64,
    index: usize,
}

impl StageNavigation {
    fn new(target: DiffTarget, revision: u64, index: usize) -> Self {
        Self { target, revision, index }
    }
}

pub struct DiffTargetChanged;

impl DiffTargetChanged {
    fn new() -> Self {
        Self
    }
}

pub struct GitDiffView {
    owner: WeakEntity<SessionView>,
    directory: PathBuf,
    target: Option<DiffTarget>,
    targets: Vec<DiffTarget>,
    state: Entity<DiffState>,
    preferred_mode: DiffMode,
    patch: Arc<String>,
    error: Option<String>,
    model: Option<Arc<DiffPatch>>,
    tool_error: Option<String>,
    selected_scope: Option<PatchScope>,
    selection_snapshot: Option<DiffLineRange>,
    selected_count: usize,
    current_hunk: Option<usize>,
    location: Option<DiffLinePosition>,
    search_input: Option<Entity<InputState>>,
    search_subscription: Option<Subscription>,
    search_open: bool,
    search_revision: u64,
    search_task: Option<Task<()>>,
    search_loading: bool,
    matches: Vec<DiffLinePosition>,
    match_index: Option<usize>,
    decorations: Vec<DiffDecoration>,
    backups: Vec<DiscardBackupEntry>,
    backup_task: Option<Task<()>>,
    loading: bool,
    pending: bool,
    revision: u64,
    task: Option<Task<()>>,
    previous_focus: Option<FocusHandle>,
    _subscription: Subscription,
}

impl EventEmitter<DiffTargetChanged> for GitDiffView {}

mod render;

impl GitDiffView {
    pub fn new(owner: WeakEntity<SessionView>, directory: PathBuf, cx: &mut Context<Self>) -> Self {
        let state = cx.new(|cx| DiffState::new(Vec::new(), cx));
        let subscription = cx.observe(&state, |this, _, cx| {
            this.selection_changed(cx);
            cx.notify();
        });
        Self {
            owner,
            directory,
            target: None,
            targets: Vec::new(),
            state,
            preferred_mode: DiffMode::default(),
            patch: Arc::new(String::new()),
            error: None,
            model: None,
            tool_error: None,
            selected_scope: None,
            selection_snapshot: None,
            selected_count: 0,
            current_hunk: None,
            location: None,
            search_input: None,
            search_subscription: None,
            search_open: false,
            search_revision: 0,
            search_task: None,
            search_loading: false,
            matches: Vec::new(),
            match_index: None,
            decorations: Vec::new(),
            backups: Vec::new(),
            backup_task: None,
            loading: false,
            pending: false,
            revision: 0,
            task: None,
            previous_focus: None,
            _subscription: subscription,
        }
    }

    pub fn is_open(&self) -> bool {
        self.target.is_some()
    }

    pub fn target(&self) -> Option<&DiffTarget> {
        self.target.as_ref()
    }

    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        let handle = self.state.read(cx).focus_handle(cx);
        handle.focus(window, cx);
    }

    pub fn is_focused(&self, window: &Window, cx: &App) -> bool {
        self.state.read(cx).focus_handle(cx).is_focused(window)
    }

    pub fn is_selected(&self, path: &Path, group: ChangeGroup) -> bool {
        self.target.as_ref().is_some_and(|target| target.path == path && target.group == group)
    }

    pub fn set_directory(&mut self, directory: &Path, cx: &mut Context<Self>) {
        if self.directory == directory {
            return;
        }
        self.directory = directory.to_owned();
        let was_open = self.is_open();
        self.target = None;
        self.targets.clear();
        self.previous_focus = None;
        self.reset(cx);
        if was_open {
            cx.emit(DiffTargetChanged::new());
        }
    }

    pub fn open(
        &mut self,
        path: PathBuf,
        group: ChangeGroup,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.is_open() {
            self.previous_focus = window.focused(cx);
        }
        if !self.is_selected(&path, group) {
            self.target = Some(DiffTarget::new(path, group));
            self.reset(cx);
            self.load_backups(window, cx);
            cx.emit(DiffTargetChanged::new());
        }
        self.initialize_search(window, cx);
        self.refresh(cx);
        self.state.focus_handle(cx).focus(window, cx);
    }

    pub fn reset(&mut self, cx: &mut Context<Self>) {
        self.revision += 1;
        self.task = None;
        self.loading = false;
        self.pending = false;
        self.patch = Arc::new(String::new());
        self.error = None;
        self.reset_tools();
        self.state.update(cx, |state, cx| state.set_files(Vec::new(), cx));
        cx.notify();
    }

    pub fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.target = None;
        self.reset(cx);
        cx.emit(DiffTargetChanged::new());
        if let Some(focus) = self.previous_focus.take() {
            window.focus(&focus, cx);
        }
    }

    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        let Some(target) = self.target.clone() else {
            return;
        };
        if self.loading {
            self.pending = true;
            return;
        }
        self.loading = true;
        self.pending = false;
        let directory = self.directory.clone();
        let previous_patch = self.patch.clone();
        let revision = self.revision;
        self.task = Some(cx.spawn(async move |view, cx| {
            let result = cx
                .background_spawn(async move {
                    let patch = read_file_diff(&directory, &target.path, target.group)?;
                    if patch == *previous_patch {
                        return Ok::<_, DiffLoadError>(None);
                    }
                    let files = DiffFile::parse(&patch)
                        .map_err(|error| DiffLoadError::display_patch(&target.path, error))?;
                    let model = if patch.is_empty() {
                        Ok(None)
                    } else {
                        DiffPatch::parse(&patch).map(|model| Some(Arc::new(model)))
                    };
                    Ok(Some((Arc::new(patch), files, model)))
                })
                .await;
            let _ = view.update(cx, |this, cx| {
                if this.revision != revision {
                    return;
                }
                this.loading = false;
                match result {
                    Ok(result) => {
                        if let Some((patch, files, model)) = result {
                            let mode = if files
                                .first()
                                .is_some_and(|file| file.status() == DiffFileStatus::Added)
                            {
                                DiffMode::Unified
                            } else {
                                this.preferred_mode
                            };
                            this.state.update(cx, |state, cx| {
                                state.set_files(files, cx);
                                state.set_mode(mode, cx);
                            });
                            this.patch = patch;
                            match model {
                                Ok(model) => {
                                    this.current_hunk = model
                                        .as_ref()
                                        .filter(|model| !model.hunks().is_empty())
                                        .map(|_| 0);
                                    this.model = model;
                                    this.tool_error = None;
                                },
                                Err(error) => {
                                    this.model = None;
                                    this.current_hunk = None;
                                    this.tool_error =
                                        Some(format!("部分操作不可用：{}", details(&error)));
                                },
                            }
                            this.location = None;
                            this.selection_snapshot = None;
                            this.selected_scope = None;
                            this.selected_count = 0;
                            this.selection_changed(cx);
                            this.update_search(cx);
                        }
                        this.error = None;
                    },
                    Err(error) => this.error = Some(details(&error)),
                }
                if this.pending {
                    this.refresh(cx);
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    pub fn set_targets(&mut self, info: Option<&GitInfo>, cx: &mut Context<Self>) {
        let files = info.and_then(GitInfo::changed_files).unwrap_or_default();
        let targets = [ChangeGroup::Staged, ChangeGroup::Unstaged]
            .into_iter()
            .flat_map(|group| {
                files.iter().filter_map(move |file| {
                    file.change(group)
                        .map(|change| DiffTarget::new(change.path().to_owned(), group))
                })
            })
            .collect::<Vec<_>>();
        if self.targets != targets {
            self.targets = targets;
            cx.notify();
        }
    }

    pub fn stage_navigation(&self, path: &Path) -> Option<StageNavigation> {
        let target = self.target.as_ref()?;
        if target.group != ChangeGroup::Unstaged || target.path != path {
            return None;
        }
        let index = self.group_targets(target.group).position(|candidate| candidate == target)?;
        Some(StageNavigation::new(target.clone(), self.revision, index))
    }

    pub fn advance_after_stage(
        &mut self,
        navigation: StageNavigation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.revision != navigation.revision
            || self.target.as_ref() != Some(&navigation.target)
            || self.targets.contains(&navigation.target)
        {
            return;
        }
        let next = self
            .group_targets(ChangeGroup::Unstaged)
            .nth(navigation.index)
            .or_else(|| self.group_targets(ChangeGroup::Unstaged).last())
            .cloned();
        if let Some(target) = next {
            self.open(target.path, target.group, window, cx);
        }
    }

    pub fn group_targets(&self, group: ChangeGroup) -> impl Iterator<Item = &DiffTarget> {
        self.targets.iter().filter(move |target| target.group == group)
    }

    pub fn adjacent_target(&self, offset: isize) -> Option<&DiffTarget> {
        let target = self.target.as_ref()?;
        let ix = self.group_targets(target.group).position(|candidate| candidate == target);
        let ix = match ix {
            Some(ix) => ix.checked_add_signed(offset)?,
            None if offset > 0 => 0,
            None => return None,
        };
        self.group_targets(target.group).nth(ix)
    }

    pub fn switch_file(&mut self, target: DiffTarget, window: &mut Window, cx: &mut Context<Self>) {
        if !self.is_open() || self.controls_busy(cx) || !self.targets.contains(&target) {
            return;
        }
        self.open(target.path, target.group, window, cx);
    }

    pub fn move_file(&mut self, offset: isize, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(target) = self.adjacent_target(offset).cloned() {
            self.switch_file(target, window, cx);
        }
    }

    pub fn open_source(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(target) = self.target.as_ref() else {
            return;
        };
        let path = self
            .state
            .read(cx)
            .files()
            .first()
            .and_then(|file| file.modified_path())
            .map(|path| PathBuf::from(path.as_str()))
            .unwrap_or_else(|| target.path.clone());
        let line = self
            .location
            .as_ref()
            .filter(|position| {
                target.group == ChangeGroup::Unstaged && position.side() == DiffSide::Modified
            })
            .and_then(|position| NonZeroUsize::new(position.line()));
        let Some(host) = self.owner.upgrade().and_then(|owner| owner.read(cx).tab_host()) else {
            return;
        };
        let Some(navigation_revision) =
            host.upgrade().map(|host| host.read(cx).file_navigation_revision())
        else {
            return;
        };
        let directory = self.directory.clone();
        let revision = self.revision;
        let task = cx.background_spawn(async move {
            let repository = git2::Repository::discover(&directory).map_err(|source| {
                crate::workspace::git::error::RepositoryError::git(
                    "discovering Git repository",
                    &directory,
                    source,
                )
            })?;
            Ok::<_, crate::workspace::git::error::RepositoryError>(
                repository.workdir().unwrap_or(&directory).to_owned(),
            )
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            let _ = cx.update(|window, cx| {
                this.update(cx, |this, cx| {
                    if this.revision != revision || !this.is_open() {
                        return;
                    }
                    match result {
                        Ok(directory) => {
                            let target = OpenFile::new(directory, path);
                            let target = match line {
                                Some(line) => target.with_line(line),
                                None => target,
                            };
                            let owner = this.owner.clone();
                            window.defer(cx, move |window, cx| {
                                let _ = host.update(cx, |host, cx| {
                                    if host.file_navigation_revision() == navigation_revision
                                        && host.is_active_session(&owner)
                                    {
                                        host.open_file(target, window, cx);
                                    }
                                });
                            });
                        },
                        Err(error) => {
                            this.tool_error = Some(details(&error));
                            cx.notify();
                        },
                    }
                })
            });
        })
        .detach();
    }

    pub fn is_added_file(&self, cx: &App) -> bool {
        self.state
            .read(cx)
            .files()
            .first()
            .is_some_and(|file| file.status() == DiffFileStatus::Added)
    }

    pub fn set_mode(&mut self, mode: DiffMode, cx: &mut Context<Self>) {
        if self.is_added_file(cx) {
            return;
        }
        self.preferred_mode = mode;
        self.state.update(cx, |state, cx| state.set_mode(mode, cx));
    }
}
