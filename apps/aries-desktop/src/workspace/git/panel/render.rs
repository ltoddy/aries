use crate::workspace::git::panel::*;

impl GitPanel {
    pub fn render(
        &self,
        view: &Entity<SessionView>,
        window: &mut Window,
        cx: &mut Context<SessionView>,
    ) -> AnyElement {
        let now = cx.background_executor().now();
        if self.motion.summary().active(now)
            || self.motion.all_rows().iter().any(|row| {
                row.visibility().active(now) || row.status().active(now) || row.lines().active(now)
            })
        {
            window.request_animation_frame();
        }
        let summary = self
            .read_error
            .as_ref()
            .map(|error| format!("无法读取 Git：{error}"))
            .unwrap_or_else(|| self.info.as_ref().map(GitInfo::summary).unwrap_or_default());
        let summary_opacity = self.motion.summary().sample(now);
        let mut panel = div()
            .id("sidebar-git-changes")
            .size_full()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .gap_2()
            .px_2()
            .py_1()
            .child(
                div()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child("Git 变更"))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(
                                Button::new("refresh-git-info")
                                    .ghost()
                                    .small()
                                    .icon(IconName::RefreshCw)
                                    .loading(self.loading)
                                    .disabled(self.index_busy)
                                    .accessibility_label("刷新 Git 变更")
                                    .tooltip("刷新 Git 变更")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.with_git_panel(|panel| panel.watch_info(cx));
                                    })),
                            )
                            .child(
                                Button::new("recover-git-discard")
                                    .ghost()
                                    .small()
                                    .icon(IconName::Undo)
                                    .disabled(self.index_busy)
                                    .accessibility_label("恢复丢弃的变更")
                                    .tooltip("恢复丢弃的变更…")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.with_git_panel(|panel| {
                                            panel.open_discard_backups(window, cx)
                                        });
                                    })),
                            ),
                    ),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .relative()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(div().whitespace_normal().opacity(summary_opacity).child(summary))
                    .when_some(self.motion.previous_summary(), |summary, previous| {
                        summary.child(
                            div()
                                .absolute()
                                .top_0()
                                .left_0()
                                .w_full()
                                .whitespace_normal()
                                .opacity(1. - summary_opacity)
                                .child(previous),
                        )
                    }),
            );
        if let Some(error) = &self.watch_error {
            panel = panel.child(
                div()
                    .flex_shrink_0()
                    .text_xs()
                    .text_color(cx.theme().warning)
                    .whitespace_normal()
                    .child(format!("自动刷新不可用：{error}。请手动刷新重试。")),
            );
        }
        if self.info.is_none() && self.read_error.is_none() {
            return panel
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .whitespace_normal()
                        .child("当前目录不在 Git 仓库中"),
                )
                .into_any_element();
        }
        if self.motion.all_rows().is_empty() {
            return panel
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .whitespace_normal()
                        .child("没有未提交的变更"),
                )
                .into_any_element();
        }
        panel
            .child(GitChangeList::new(self, view, ChangeGroup::Staged).render(cx))
            .child(GitChangeList::new(self, view, ChangeGroup::Unstaged).render(cx))
            .into_any_element()
    }

    pub fn open_discard_backups(&self, window: &mut Window, cx: &mut Context<SessionView>) {
        let directory = self.cwd.clone();
        cx.spawn_in(window, async move |view, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let repository = git2::Repository::discover(&directory).map_err(|source| {
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
                            Ok(entry) if entry.root() == root => backups.push(entry),
                            Ok(_) => {},
                            Err(error) => {
                                log("无法读取备份记录", &error);
                                errors.push(details(&error));
                            },
                        }
                    }
                    let error = (!errors.is_empty())
                        .then(|| GitOperationError::backup_entries(errors.len(), errors.join("\n")));
                    Ok::<_, GitOperationError>((backups, error, directory))
                })
                .await;
            let (backups, error, directory) = match result {
                Ok(result) => result,
                Err(error) => {
                    if view
                        .update_in(cx, |_, window, cx| report("无法读取备份", &error, window, cx))
                        .is_err()
                    {
                        log("无法读取备份", &error);
                    }
                    return;
                },
            };
            let _ = view.update_in(cx, |session, window, cx| {
                if session.git_panel().directory() != directory {
                    return;
                }
                if let Some(error) = &error {
                    report("部分备份无法读取", error, window, cx);
                }
                let error = error.as_ref().map(|error| details(error));
                let owner = cx.weak_entity();
                window.open_dialog(cx, move |dialog, _, _| {
                    let backups = backups.clone();
                    let error = error.clone();
                    let owner = owner.clone();
                    let directory = directory.clone();
                    dialog.title("恢复丢弃").content(move |content, _, cx| {
                        let mut list = div()
                            .id("discard-backup-list")
                            .flex()
                            .flex_col()
                            .gap_2()
                            .max_h(rems(24.))
                            .overflow_y_scroll();
                        if let Some(error) = &error {
                            list = list.child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().warning)
                                    .whitespace_normal()
                                    .child(error.clone()),
                            );
                        }
                        if backups.is_empty() {
                            list = list.child(if error.is_some() {
                                "当前仓库没有可读取的恢复记录；部分备份无法读取，不能确认没有丢弃记录。"
                            } else {
                                "没有可恢复的丢弃记录"
                            });
                        }
                        for backup in backups.iter().rev() {
                            let backup = backup.clone();
                            let owner = owner.clone();
                            let directory = directory.clone();
                            let label = SharedString::from(format!(
                                "{} · {}",
                                backup.path().display(),
                                backup
                                    .directory()
                                    .file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy()
                            ));
                            list = list.child(
                                Button::new(label.clone())
                                    .label(label)
                                    .on_click(move |_, window, cx| {
                                        let backup = backup.clone();
                                        let owner = owner.clone();
                                        let directory = directory.clone();
                                        window.close_dialog(cx);
                                        window.defer(cx, move |window, cx| {
                                            let _ = owner.update(cx, |session, cx| {
                                                session.with_git_panel(|panel| {
                                                    if panel.directory() == directory {
                                                        panel.restore_diff(backup, window, cx);
                                                    }
                                                });
                                            });
                                        });
                                    }),
                            );
                        }
                        content.child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_3()
                            .child(div().text_sm().text_color(cx.theme().muted_foreground).child(
                                "选择要恢复的记录。文件已变化时会拒绝覆盖；操作期间请暂停编辑。",
                            ))
                            .child(list),
                    )
                    })
                });
            });
        })
        .detach();
    }
}
