use std::time::Duration;

use crate::workspace::git::diff_view::DiffTarget;
use crate::workspace::git::panel::*;
use crate::workspace::git::repository::FileChange;

fn target_change<'a>(info: Option<&'a GitInfo>, target: &DiffTarget) -> Option<&'a FileChange> {
    info?
        .changed_files()?
        .iter()
        .filter_map(|file| file.change(target.group()))
        .find(|change| change.path() == target.path())
}

impl GitPanel {
    pub fn sync_info(
        &mut self,
        result: Result<Option<GitInfo>, RepositoryError>,
        animate: bool,
        cx: &mut Context<SessionView>,
    ) {
        if self.motion.reset(&self.cwd) {
            self.info = None;
            self.read_error = None;
        }
        let (info, error) = match result {
            Ok(None) if self.info.is_some() || self.read_error.is_some() => {
                (None, Some("项目不再是可读取的 Git 仓库".into()))
            },
            Ok(info) => (info, None),
            Err(error) => (None, Some(details(&error))),
        };
        let changed = self.info != info || self.read_error != error;
        let touched = std::mem::take(&mut self.touched);
        let target = self.diff.read(cx).target().cloned();
        let reload_diff = target.as_ref().is_none_or(|target| {
            touched.is_empty()
                || touched.iter().any(|path| path.ends_with(target.path()))
                || target_change(self.info.as_ref(), target) != target_change(info.as_ref(), target)
        });
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
        if changed || reload_diff {
            self.diff.update(cx, |diff, cx| {
                if changed {
                    diff.set_targets(self.info.as_ref(), cx);
                }
                if reload_diff {
                    diff.refresh_silent(cx);
                }
            });
        }
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
        if !self.is_index_action_available(&path, action) {
            return;
        }
        self.index_busy = true;
        self.pending_index = Some((path.clone(), action));
        self.diff.update(cx, |_, cx| cx.notify());
        self.revision += 1;
        let revision = self.revision;
        let directory = self.cwd.clone();
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
            let _ = view.update(cx, |this, cx| {
                this.with_git_panel(|panel| {
                    panel.index_busy = false;
                    panel.pending_index = None;
                    panel.diff.update(cx, |_, cx| cx.notify());
                    let current = accept_result(&panel.cwd, panel.revision, &directory, revision);
                    if current {
                        panel.sync_info(info, result.is_ok(), cx);
                    }
                    panel.revision += 1;
                    if panel.refresh_pending || !current {
                        panel.refresh_info(false, cx);
                    }
                    cx.notify();
                })
            });
            let label = match action {
                IndexAction::Stage => "无法暂存",
                IndexAction::Unstage => "无法取消暂存",
            };
            let title = format!("{label} {}", path.display());
            let delivered = view.update_in(cx, |_, window, cx| {
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
