use crate::workspace::git::panel::*;

impl GitPanel {
    pub fn change_diff(
        &mut self,
        request: DiffRequest,
        window: &mut Window,
        cx: &mut Context<SessionView>,
    ) {
        if request.directory() != self.cwd {
            return;
        }
        let path = request.path().to_owned();
        let navigation = (request.action() == DiffAction::stage())
            .then(|| self.diff.read(cx).stage_navigation(&path))
            .flatten();
        self.run_diff_operation(path, move || request.execute(), navigation, window, cx);
    }

    pub fn restore_diff(
        &mut self,
        backup: DiscardBackupEntry,
        window: &mut Window,
        cx: &mut Context<SessionView>,
    ) {
        let directory = self.cwd.clone();
        let path = backup.path().to_owned();
        self.run_diff_operation(
            path,
            move || {
                with_git_write(|| {
                    let repository = git2::Repository::discover(&directory).map_err(|source| {
                        RepositoryError::git("discovering backup repository", &directory, source)
                    })?;
                    let root = std::fs::canonicalize(
                        repository.workdir().ok_or_else(GitOperationError::no_workdir)?,
                    )
                    .map_err(GitOperationError::resolving_repository_workdir)?;
                    if root != backup.root() {
                        return Err(GitOperationError::backup_repository_mismatch());
                    }
                    backup.load()?.restore()?;
                    Ok(None)
                })
            },
            None,
            window,
            cx,
        );
    }

    pub fn run_diff_operation(
        &mut self,
        path: PathBuf,
        operation: impl FnOnce() -> Result<Option<DiscardBackup>, GitOperationError> + Send + 'static,
        navigation: Option<StageNavigation>,
        window: &mut Window,
        cx: &mut Context<SessionView>,
    ) {
        if self.index_busy {
            return;
        }
        self.index_busy = true;
        self.revision += 1;
        let revision = self.revision;
        let directory = self.cwd.clone();
        self.diff.update(cx, |_, cx| cx.notify());
        cx.spawn_in(window, async move |view, cx| {
            let (result, info, directory) = cx
                .background_executor()
                .spawn(async move {
                    let result = operation();
                    let info = GitInfo::read(&directory);
                    (result, info, directory)
                })
                .await;
            let _ = view.update_in(cx, |session, window, cx| {
                session.with_git_panel(|panel| {
                    panel.index_busy = false;
                    let current = accept_result(&panel.cwd, panel.revision, &directory, revision);
                    if current {
                        panel.sync_info(info, result.is_ok(), cx);
                    }
                    panel.diff.update(cx, |diff, cx| {
                        if current
                            && result.is_ok()
                            && panel.info.as_ref().and_then(GitInfo::changed_files).is_some()
                            && let Some(navigation) = navigation
                        {
                            diff.advance_after_stage(navigation, window, cx);
                        }
                        diff.refresh(cx);
                        cx.notify();
                    });
                    panel.revision += 1;
                    if panel.refresh_pending || !current {
                        panel.refresh_info(cx);
                    }
                    cx.notify();
                });
            });
            let title = format!("无法处理 {}", path.display());
            let delivered = view.update_in(cx, |session, window, cx| {
                session.git_panel().diff.update(cx, |diff, cx| diff.load_backups(window, cx));
                match &result {
                    Ok(Some(_)) => window.push_notification(
                        Notification::success(format!(
                            "已丢弃 {} 的所选变更，可从「恢复丢弃」恢复",
                            path.display()
                        )),
                        cx,
                    ),
                    Ok(None) => {},
                    Err(error) => report(title.clone(), error, window, cx),
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
