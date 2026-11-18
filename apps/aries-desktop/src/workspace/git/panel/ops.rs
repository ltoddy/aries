use crate::workspace::git::panel::*;

impl GitPanel {
    pub fn set_collapsed(
        &mut self,
        collapsed: bool,
        window: &mut Window,
        cx: &mut Context<SessionView>,
    ) {
        if self.info_collapsed == collapsed {
            return;
        }
        self.info_collapsed = collapsed;
        let tokens = cx.theme().motion_tokens();
        let duration = if cx.reduce_motion() { Duration::ZERO } else { tokens.duration_fast };
        self.motion.panel_mut().retarget(
            if collapsed { 0. } else { 1. },
            cx.background_executor().now(),
            duration,
            if collapsed { tokens.easing_exit.clone() } else { tokens.easing_enter.clone() },
        );
        if collapsed {
            self.trigger_focus.focus(window, cx);
        } else {
            self.panel_focus.focus(window, cx);
        }
        cx.notify();
    }

    pub fn sync_info(
        &mut self,
        result: Result<Option<GitInfo>, RepositoryError>,
        animate: bool,
        cx: &mut Context<SessionView>,
    ) {
        if self.motion.reset(&self.cwd) {
            self.info = None;
            self.read_error = None;
            self.info_collapsed = true;
        }
        let (info, error) = match result {
            Ok(None) if self.info.is_some() || self.read_error.is_some() => {
                (None, Some("项目不再是可读取的 Git 仓库".into()))
            },
            Ok(info) => (info, None),
            Err(error) => (None, Some(details(&error))),
        };
        let changed = self.info != info || self.read_error != error;
        let tokens = cx.theme().motion_tokens();
        let duration = if cx.reduce_motion() { Duration::ZERO } else { tokens.duration_fast };
        self.motion.sync(
            info.as_ref(),
            cx.background_executor().now(),
            duration,
            tokens.easing_enter.clone(),
            animate,
        );
        self.info = info;
        self.read_error = error;
        self.diff.update(cx, |diff, cx| {
            diff.set_targets(self.info.as_ref(), cx);
            diff.refresh(cx);
        });
        if changed {
            if !duration.is_zero() {
                cx.spawn(async move |view, cx| {
                    cx.background_executor().timer(duration).await;
                    let _ = view.update(cx, |this, cx| {
                        this.with_git_panel(|panel| panel.cleanup(cx));
                        cx.notify();
                    });
                })
                .detach();
            }
            cx.notify();
        }
    }

    pub fn is_index_action_available(&self, path: &Path, action: IndexAction) -> bool {
        let group = match action {
            IndexAction::Stage => ChangeGroup::Unstaged,
            IndexAction::Unstage => ChangeGroup::Staged,
        };
        !self.index_busy
            && self.motion.rows(group).any(|row| {
                row.file().path() == path
                    && !row.is_exiting()
                    && !row.file().status().is_conflicted()
            })
    }

    pub fn change_git_index(
        &mut self,
        path: impl Into<PathBuf>,
        action: IndexAction,
        window: &mut Window,
        cx: &mut Context<SessionView>,
    ) {
        let path = path.into();
        let group = match action {
            IndexAction::Stage => ChangeGroup::Unstaged,
            IndexAction::Unstage => ChangeGroup::Staged,
        };
        if !self.is_index_action_available(&path, action) {
            return;
        }
        self.index_busy = true;
        self.pending_index = Some((path.clone(), action));
        self.diff.update(cx, |_, cx| cx.notify());
        self.revision += 1;
        let revision = self.revision;
        let directory = self.cwd.clone();
        let action_focus = window.focused(cx);
        cx.spawn_in(window, async move |view, cx| {
            let (result, info, path, directory) = cx
                .background_executor()
                .spawn(async move {
                    let result = with_git_write(|| {
                        update_index(&directory, &path, action)
                            .map_err(GitOperationError::updating_git_index)
                    });
                    let info = GitInfo::read(&directory);
                    (result, info, path, directory)
                })
                .await;
            let focus_panel = view
                .update(cx, |this, cx| {
                    this.with_git_panel(|panel| {
                        panel.index_busy = false;
                        panel.pending_index = None;
                        panel.diff.update(cx, |_, cx| cx.notify());
                        let current =
                            accept_result(&panel.cwd, panel.revision, &directory, revision);
                        if current {
                            panel.sync_info(info, result.is_ok(), cx);
                        }
                        let focus_panel = current
                            && !panel.info_collapsed
                            && !panel
                                .motion
                                .rows(group)
                                .any(|row| row.file().path() == path && !row.is_exiting());
                        panel.revision += 1;
                        if panel.refresh_pending || !current {
                            panel.refresh_info(cx);
                        }
                        cx.notify();
                        focus_panel
                    })
                })
                .unwrap_or(false);
            let label = match action {
                IndexAction::Stage => "无法暂存",
                IndexAction::Unstage => "无法取消暂存",
            };
            let title = format!("{label} {}", path.display());
            let delivered = view.update_in(cx, |this, window, cx| {
                this.with_git_panel(|panel| {
                    if focus_panel && window.focused(cx) == action_focus {
                        panel.panel_focus.focus(window, cx);
                    }
                });
                if let Err(error) = &result {
                    report(title.clone(), error, window, cx);
                }
            });
            if delivered.is_err()
                && let Err(error) = &result
            {
                log(&title, error);
            }
        })
        .detach();
        cx.notify();
    }
}
