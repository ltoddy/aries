use std::path::{Path, PathBuf};
use std::time::Duration;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::{ActiveTheme, Disableable, Sizable, WindowExt};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::error::{details, log, report};
use crate::workspace::SessionView;
use crate::workspace::git::diff_operations::{
    DiffAction, DiffRequest, DiscardBackup, DiscardBackupEntry, GitOperationError, with_git_write,
};
use crate::workspace::git::diff_view::{DiffTargetChanged, GitDiffView, StageNavigation};
use crate::workspace::git::error::RepositoryError;
use crate::workspace::git::index::{IndexAction, update_index};
use crate::workspace::git::list::GitChangeList;
use crate::workspace::git::motion::{GitMotion, accept_result};
use crate::workspace::git::repository::{ChangeGroup, GitInfo};
use crate::workspace::git::watch::GitWatch;

mod diff;
mod ops;
mod render;

pub struct GitPanel {
    cwd: PathBuf,
    info: Option<GitInfo>,
    info_task: Option<Task<()>>,
    refresh_task: Option<Task<()>>,
    reading: bool,
    loading: bool,
    refresh_pending: bool,
    watch_error: Option<String>,
    index_busy: bool,
    revision: u64,
    motion: GitMotion,
    pending_index: Option<(PathBuf, IndexAction)>,
    touched: Vec<PathBuf>,
    read_error: Option<String>,
    diff: Entity<GitDiffView>,
    _diff_subscription: Subscription,
}

impl GitPanel {
    pub fn new(cwd: impl Into<PathBuf>, cx: &mut Context<SessionView>) -> Self {
        let cwd = cwd.into();
        let owner = cx.weak_entity();
        let diff = cx.new(|cx| GitDiffView::new(owner, cwd.clone(), cx));
        let diff_subscription = cx.subscribe(&diff, |_, _, _: &DiffTargetChanged, cx| cx.notify());
        Self {
            cwd,
            info: None,
            info_task: None,
            refresh_task: None,
            reading: false,
            loading: false,
            refresh_pending: false,
            watch_error: None,
            index_busy: false,
            revision: 0,
            motion: GitMotion::default(),
            pending_index: None,
            touched: Vec::new(),
            read_error: None,
            diff,
            _diff_subscription: diff_subscription,
        }
    }
    pub fn set_directory(&mut self, cwd: impl Into<PathBuf>) {
        self.cwd = cwd.into();
    }
    pub fn diff(&self) -> &Entity<GitDiffView> {
        &self.diff
    }
    pub fn open_diff(
        &mut self,
        path: PathBuf,
        group: ChangeGroup,
        window: &mut Window,
        cx: &mut Context<SessionView>,
    ) {
        self.diff.update(cx, |diff, cx| {
            diff.set_targets(self.info.as_ref(), cx);
            diff.open(path, group, window, cx);
        });
        let owner = cx.weak_entity();
        window.defer(cx, move |window, cx| {
            if let Some(host) = owner.upgrade().and_then(|owner| owner.read(cx).tab_host()) {
                let _ = host.update(cx, |host, cx| host.show_diff(&owner, window, cx));
            }
        });
    }
    pub fn directory(&self) -> &Path {
        &self.cwd
    }
    pub fn motion(&self) -> &GitMotion {
        &self.motion
    }
    pub fn is_index_busy(&self) -> bool {
        self.index_busy
    }
    pub fn pending_index(&self) -> Option<&(PathBuf, IndexAction)> {
        self.pending_index.as_ref()
    }
    pub fn info(&self) -> Option<&GitInfo> {
        self.info.as_ref()
    }
    pub fn cleanup(&mut self, cx: &App) {
        self.motion.cleanup(cx.background_executor().now(), cx.reduce_motion());
    }

    pub fn watch_info(&mut self, cx: &mut Context<SessionView>) {
        self.diff.update(cx, |diff, cx| diff.set_directory(&self.cwd, cx));
        if self.motion.reset(&self.cwd) {
            self.info = None;
            self.read_error = None;
        }
        self.revision += 1;
        self.refresh_task = None;
        self.reading = false;
        self.loading = false;
        self.refresh_pending = false;
        self.watch_error = None;
        let directory = self.cwd.clone();
        self.info_task = Some(cx.spawn(async move |view, cx| {
            let (sender, receiver) = async_channel::bounded(1);
            let executor = cx.background_executor().clone();
            let watcher = cx
                .background_spawn(async move { GitWatch::start(&directory, sender, executor) })
                .await;
            let watcher = match watcher {
                Ok(watcher) => watcher,
                Err(error) => {
                    let _ = view.update(cx, |this, cx| {
                        this.with_git_panel(|panel| {
                            log("Git 自动刷新不可用", &error);
                            panel.watch_error = Some(details(&error));
                            panel.refresh_info(true, cx);
                        });
                    });
                    return;
                },
            };
            if view
                .update(cx, |this, cx| this.with_git_panel(|panel| panel.refresh_info(true, cx)))
                .is_err()
            {
                return;
            }
            while receiver.recv().await.is_ok() {
                cx.background_executor().timer(Duration::from_millis(300)).await;
                while receiver.try_recv().is_ok() {}
                let (error, poisoned) = match watcher.take_error() {
                    Ok(error) => (error, false),
                    Err(error) => (Some(error), true),
                };
                let touched = watcher.take_paths();
                if view
                    .update(cx, |this, cx| {
                        this.with_git_panel(|panel| {
                            panel.touched.extend(touched);
                            if let Some(error) = error {
                                let message = details(&error);
                                if panel.watch_error.as_ref() != Some(&message) {
                                    log("Git 自动刷新不可用", &error);
                                    panel.watch_error = Some(message);
                                }
                            }
                            panel.refresh_info(false, cx);
                        });
                    })
                    .is_err()
                {
                    return;
                }
                if poisoned {
                    break;
                }
            }
            drop(watcher);
        }));
        cx.notify();
    }

    pub fn refresh_info(&mut self, show_loading: bool, cx: &mut Context<SessionView>) {
        if show_loading {
            self.loading = true;
        }
        if self.reading || self.index_busy {
            self.refresh_pending = true;
            return;
        }
        self.reading = true;
        self.refresh_pending = false;
        self.refresh_task = Some(cx.spawn(async move |view, cx| {
            loop {
                let Ok((directory, revision)) = view.read_with(cx, |this, _| {
                    (this.git_panel().cwd.clone(), this.git_panel().revision)
                }) else {
                    return;
                };
                let (info, directory) = cx
                    .background_spawn(async move { (GitInfo::read(&directory), directory) })
                    .await;
                let again = view.update(cx, |this, cx| {
                    this.with_git_panel(|panel| {
                        if accept_result(&panel.cwd, panel.revision, &directory, revision)
                            && !panel.index_busy
                        {
                            panel.sync_info(info, true, cx);
                        }
                        let again = panel.refresh_pending && !panel.index_busy;
                        if again {
                            panel.refresh_pending = false;
                        } else {
                            panel.reading = false;
                            panel.loading = false;
                        }
                        cx.notify();
                        again
                    })
                });
                if !matches!(again, Ok(true)) {
                    return;
                }
            }
        }));
        if show_loading {
            cx.notify();
        }
    }
}
