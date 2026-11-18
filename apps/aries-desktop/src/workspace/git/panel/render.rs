use crate::workspace::git::panel::*;

impl GitPanel {
    pub fn render(
        &self,
        view: &Entity<SessionView>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<AnyElement> {
        let conversation = self;
        if conversation.info.is_none()
            && conversation.read_error.is_none()
            && conversation.watch_error.is_none()
        {
            return None;
        }
        let summary = conversation
            .read_error
            .as_ref()
            .map(|error| format!("无法读取 Git：{error}"))
            .unwrap_or_else(|| {
                conversation.info.as_ref().map(GitInfo::summary).unwrap_or_default()
            });
        let now = cx.background_executor().now();
        let expanded = !conversation.info_collapsed;
        let progress = conversation.motion.panel().sample(now);
        if conversation.motion.panel().active(now)
            || conversation.motion.summary().active(now)
            || conversation.motion.all_rows().iter().any(|row| {
                row.visibility().active(now) || row.status().active(now) || row.lines().active(now)
            })
        {
            window.request_animation_frame();
        }
        let id: SharedString =
            format!("project-git-{:?}-{}", view.entity_id(), conversation.cwd.display()).into();
        let ball = if progress < 1. || !expanded {
            let view = view.clone();
            Some(
                div()
                    .id((ElementId::from(id.clone()), "ball"))
                    .absolute()
                    .top_3()
                    .right_3()
                    .rounded(cx.theme().radius_full())
                    .shadow_sm()
                    .opacity(1. - progress)
                    .when(!expanded, |ball| ball.occlude())
                    .child(if expanded {
                        div()
                            .h_8()
                            .w_10()
                            .rounded(cx.theme().radius_full())
                            .bg(cx.theme().popover)
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(Icon::new(IconName::GitBranch).size_4())
                            .into_any_element()
                    } else {
                        Button::new("expand-git-info")
                            .ghost()
                            .small()
                            .h_8()
                            .w_10()
                            .rounded(cx.theme().radius_full())
                            .bg(cx.theme().popover)
                            .text_color(cx.theme().muted_foreground)
                            .icon(IconName::GitBranch)
                            .track_focus(&conversation.trigger_focus)
                            .disabled(expanded)
                            .accessibility_label("展开 Git 变更")
                            .tooltip(format!("展开 Git 变更 · {summary}"))
                            .on_click(move |_, window, cx| {
                                view.update(cx, |this, cx| {
                                    this.with_git_panel(|panel| {
                                        panel.set_collapsed(false, window, cx)
                                    })
                                });
                            })
                            .into_any_element()
                    })
                    .into_any_element(),
            )
        } else {
            None
        };
        if progress == 0. && !expanded {
            return ball;
        }
        let mut panel = div()
            .id(id)
            .absolute()
            .top_3()
            .right_3()
            .w(rems(20.))
            .max_w(relative(0.9))
            .p_2()
            .rounded(cx.theme().radius_tokens().lg)
            .bg(cx.theme().popover)
            .text_color(cx.theme().popover_foreground)
            .shadow_sm()
            .opacity(progress)
            .when(expanded, |panel| panel.occlude())
            .mt(cx.theme().motion_tokens().distance_short * (1. - progress))
            .flex()
            .flex_col()
            .gap_2()
            .min_w_0()
            .aria_label("Git 变更文件")
            .when(expanded, |panel| {
                let view = view.clone();
                panel.on_key_down(move |event, window, cx| {
                    if event.keystroke.key == "escape" {
                        view.update(cx, |this, cx| {
                            this.with_git_panel(|panel| panel.set_collapsed(true, window, cx))
                        });
                        cx.stop_propagation();
                    }
                })
            })
            .child(
                div()
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
                            .when(expanded, |actions| {
                                let view = view.clone();
                                actions.child(
                                    Button::new("refresh-git-info")
                                        .ghost()
                                        .small()
                                        .icon(IconName::RefreshCw)
                                        .loading(conversation.refreshing)
                                        .disabled(conversation.index_busy)
                                        .accessibility_label("刷新 Git 变更")
                                        .tooltip("刷新 Git 变更")
                                        .on_click(move |_, _, cx| {
                                            view.update(cx, |this, cx| {
                                                this.with_git_panel(|panel| panel.watch_info(cx));
                                            });
                                        }),
                                )
                            })
                            .when(expanded, |actions| {
                                let view = view.clone();
                                actions.child(
                                    Button::new("recover-git-discard")
                                        .ghost()
                                        .small()
                                        .icon(IconName::Undo)
                                        .disabled(conversation.index_busy)
                                        .accessibility_label("恢复丢弃的变更")
                                        .tooltip("恢复丢弃的变更…")
                                        .on_click(move |_, window, cx| {
                                            view.update(cx, |this, cx| {
                                                this.with_git_panel(|panel| {
                                                    panel.open_discard_backups(window, cx)
                                                });
                                            });
                                        }),
                                )
                            })
                            .child(if !expanded {
                                div()
                                    .size_6()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(Icon::new(IconName::ChevronUp).size_4())
                                    .into_any_element()
                            } else {
                                let view = view.clone();
                                let label = "缩小为悬浮球";
                                Button::new("toggle-git-info")
                                    .track_focus(&conversation.panel_focus)
                                    .disabled(!expanded)
                                    .ghost()
                                    .small()
                                    .icon(IconName::ChevronUp)
                                    .accessibility_label(label)
                                    .tooltip(label)
                                    .on_click(move |_, window, cx| {
                                        view.update(cx, |this, cx| {
                                            this.with_git_panel(|panel| {
                                                panel.set_collapsed(true, window, cx)
                                            })
                                        });
                                    })
                                    .into_any_element()
                            }),
                    ),
            );
        let summary_opacity = conversation.motion.summary().sample(now);
        panel = panel.child(
            div()
                .relative()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(div().whitespace_normal().opacity(summary_opacity).child(summary))
                .when_some(conversation.motion.previous_summary(), |summary, previous| {
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
        if let Some(error) = &conversation.watch_error {
            panel = panel.child(
                div()
                    .text_xs()
                    .text_color(cx.theme().warning)
                    .whitespace_normal()
                    .child(format!("自动刷新不可用：{error}。请手动刷新重试。")),
            );
        }
        if !conversation.motion.all_rows().is_empty() {
            panel = panel
                .child(
                    GitChangeList::new(self, view, ChangeGroup::Staged, window.rem_size() * 2.)
                        .render(cx),
                )
                .child(
                    GitChangeList::new(self, view, ChangeGroup::Unstaged, window.rem_size() * 2.)
                        .render(cx),
                );
        }
        Some(
            div()
                .absolute()
                .inset_0()
                .children([Some(panel.into_any_element()), ball].into_iter().flatten())
                .into_any_element(),
        )
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
                            let label = format!(
                                "{} · {}",
                                backup.path().display(),
                                backup
                                    .directory()
                                    .file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy()
                            );
                            list = list.child(
                                Button::new(SharedString::from(label.clone()))
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
