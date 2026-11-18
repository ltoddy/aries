use crate::workspace::git::diff_view::tools::*;

impl GitDiffView {
    pub fn controls_busy(&self, cx: &App) -> bool {
        self.loading
            || self.owner.upgrade().is_none_or(|owner| {
                let panel = owner.read(cx).git_panel();
                panel.directory() != self.directory || panel.is_index_busy()
            })
    }

    pub fn writes_busy(&self, cx: &App) -> bool {
        self.error.is_some() || self.controls_busy(cx)
    }

    pub fn scope_available(&self, scope: ToolScope) -> bool {
        if self.patch.is_empty() {
            return false;
        }
        if scope == ToolScope::File {
            return true;
        }
        self.model.as_ref().is_some_and(|model| model.partial_error().is_none())
            && self.scope_patch(scope).is_some()
    }

    pub fn render_write_action(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(target) = &self.target else {
            return div().into_any_element();
        };
        let (label, action) = match target.group {
            ChangeGroup::Unstaged => ("暂存", DiffAction::stage()),
            ChangeGroup::Staged => ("取消暂存", DiffAction::unstage()),
        };
        Button::new("diff-index-operation")
            .small()
            .compact()
            .secondary()
            .label(format!("{label}整个文件"))
            .disabled(self.writes_busy(cx) || !self.scope_available(ToolScope::file()))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.request_change(action, ToolScope::file(), window, cx);
            }))
            .into_any_element()
    }

    pub fn render_tools(&self, cx: &mut Context<Self>) -> AnyElement {
        let view = cx.weak_entity();
        let current = self.current_hunk;
        let count = self.model.as_ref().map_or(0, |model| model.hunks().len());
        let hunk_view = view.clone();
        let restore_view = view;
        let hunk_snapshot = self.tool_snapshot();
        let restore_snapshot = hunk_snapshot.clone();
        div()
            .flex()
            .items_center()
            .gap_1()
            .flex_1()
            .min_w_0()
            .flex_wrap()
            .child(
                Button::new("previous-diff-change")
                    .small()
                    .ghost()
                    .compact()
                    .icon(IconName::ChevronUp)
                    .accessibility_label("上一个变更")
                    .disabled(count == 0)
                    .tooltip("上一个变更块")
                    .on_click(cx.listener(|this, _, _, cx| this.move_hunk(true, cx))),
            )
            .child(
                Button::new("next-diff-change")
                    .small()
                    .ghost()
                    .compact()
                    .icon(IconName::ChevronDown)
                    .accessibility_label("下一个变更")
                    .disabled(count == 0)
                    .tooltip("下一个变更块")
                    .on_click(cx.listener(|this, _, _, cx| this.move_hunk(false, cx))),
            )
            .child(
                Button::new("diff-hunk-selector")
                    .small()
                    .ghost()
                    .compact()
                    .font_family(cx.theme().mono_font_family.clone())
                    .label(format!("块 {} / {count}", current.map_or(0, |index| index + 1)))
                    .icon(IconName::ChevronDown)
                    .disabled(count == 0)
                    .dropdown_menu(move |menu, _, cx| {
                        let Some(view) = hunk_view.upgrade() else {
                            return menu;
                        };
                        let restored = hunk_snapshot.is_current(view.read(cx));
                        let Some(model) = view.read(cx).model.clone() else {
                            return menu;
                        };
                        let mut menu = menu;
                        for hunk in model.hunks() {
                            let index = hunk.index();
                            let view = hunk_view.clone();
                            let snapshot = hunk_snapshot.clone();
                            menu = menu.item(
                                PopupMenuItem::new(format!(
                                    "{} · -{},{} +{},{} {}",
                                    index + 1,
                                    hunk.original_start(),
                                    hunk.original_count(),
                                    hunk.modified_start(),
                                    hunk.modified_count(),
                                    hunk.label()
                                ))
                                .checked(current == Some(index))
                                .disabled(!restored)
                                .on_click(move |_, _, cx| {
                                    let _ = view.update(cx, |this, cx| {
                                        if this.restore_tool_snapshot(&snapshot, cx) {
                                            this.choose_hunk(index, cx);
                                        }
                                    });
                                }),
                            );
                        }
                        menu
                    }),
            )
            .child(
                Button::new("search-diff")
                    .small()
                    .ghost()
                    .compact()
                    .icon(IconName::Search)
                    .accessibility_label("搜索 diff")
                    .tooltip("搜索 diff · Cmd/Ctrl+F")
                    .on_click(cx.listener(|this, _, window, cx| this.show_search(window, cx))),
            )
            .child(
                Button::new("restore-discarded-diff")
                    .small()
                    .ghost()
                    .compact()
                    .label("恢复丢弃")
                    .icon(IconName::ChevronDown)
                    .disabled(self.backups.is_empty() || self.controls_busy(cx))
                    .dropdown_menu(move |menu, _, cx| {
                        let Some(view) = restore_view.upgrade() else {
                            return menu;
                        };
                        let restored = restore_snapshot.is_current(view.read(cx));
                        let this = view.read(cx);
                        let mut menu = menu;
                        for backup in this.backups.iter().rev() {
                            let label = format!(
                                "{} · {}",
                                backup.path().display(),
                                backup
                                    .directory()
                                    .file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy()
                            );
                            let backup = backup.clone();
                            let owner = this.owner.clone();
                            let directory = this.directory.clone();
                            let view = restore_view.clone();
                            let snapshot = restore_snapshot.clone();
                            menu =
                                menu.item(PopupMenuItem::new(label).disabled(!restored).on_click(
                                    move |_, window, cx| {
                                        if !view
                                            .update(cx, |this, cx| {
                                                this.restore_tool_snapshot(&snapshot, cx)
                                            })
                                            .unwrap_or(false)
                                        {
                                            return;
                                        }
                                        let owner = owner.clone();
                                        let backup = backup.clone();
                                        let directory = directory.clone();
                                        window.defer(cx, move |window, cx| {
                                            let _ = owner.update(cx, |session, cx| {
                                                session.with_git_panel(|panel| {
                                                    if panel.directory() == directory {
                                                        panel.restore_diff(backup, window, cx);
                                                    }
                                                })
                                            });
                                        });
                                    },
                                ));
                        }
                        menu
                    }),
            )
            .child(div().min_w_0().text_xs().text_color(cx.theme().muted_foreground).child(
                format!(
                    "已选 {} 个变更行{} · 行号选择，右键操作",
                    self.selected_count,
                    self.location.as_ref().map_or(String::new(), |position| format!(
                        " · {}侧:{}",
                        if position.side() == DiffSide::Original { "原始" } else { "修改" },
                        position.line()
                    ))
                ),
            ))
            .into_any_element()
    }
}
