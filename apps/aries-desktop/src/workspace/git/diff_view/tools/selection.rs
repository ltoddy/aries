use crate::workspace::git::diff_view::tools::*;

impl GitDiffView {
    pub fn selection_changed(&mut self, cx: &mut Context<Self>) {
        let selection = self.state.read(cx).selected_lines();
        if self.selection_snapshot == selection {
            return;
        }
        self.selection_snapshot = selection.clone();
        self.selected_scope = None;
        self.selected_count = 0;
        let Some(range) = selection else {
            return;
        };
        if !self.state.read(cx).files().first().is_some_and(|file| file.path() == range.path()) {
            return;
        }
        let Some(model) = &self.model else {
            return;
        };
        let result = (|| -> Result<PatchScope> {
            let anchor = endpoint(range.side(), range.start())?;
            let head = endpoint(range.end_side(), range.end())?;
            let selection = LineSelection::from_endpoints(model, anchor, head)?;
            Ok(PatchScope::lines(selection))
        })();
        if let Ok(scope) = result {
            self.selected_count = model.selected_changed_line_count(&scope).unwrap_or(0);
            if self.selected_count > 0 {
                self.selected_scope = Some(scope);
            }
            let position =
                DiffLinePosition::new(range.path().clone(), range.end_side(), range.end());
            self.set_location(position, cx);
        }
    }

    pub fn set_location(&mut self, position: DiffLinePosition, cx: &mut Context<Self>) {
        if let Some(model) = &self.model
            && let Ok(endpoint) = endpoint(position.side(), position.line())
            && let Some(source) = model.source_position(endpoint)
        {
            self.current_hunk = model
                .source_lines()
                .iter()
                .find(|line| line.position() == source)
                .map(|line| line.hunk_index());
        }
        self.location = Some(position);
        cx.notify();
    }

    pub fn scope_patch(&self, scope: ToolScope) -> Option<PatchScope> {
        match scope {
            ToolScope::File => Some(PatchScope::file()),
            ToolScope::Hunk => self.current_hunk.map(PatchScope::hunk),
            ToolScope::Lines => self.selected_scope.clone(),
        }
    }

    pub fn selection_menu(&mut self, mut menu: PopupMenu, cx: &mut Context<Self>) -> PopupMenu {
        self.selection_changed(cx);
        let Some(target) = &self.target else {
            return menu;
        };
        let Some(range) = &self.selection_snapshot else {
            return menu.item(PopupMenuItem::new("先点击或拖动行号选择变更行").disabled(true));
        };
        let hunk = self.model.as_ref().and_then(|model| {
            let start = model.source_position(endpoint(range.side(), range.start()).ok()?)?;
            let end = model.source_position(endpoint(range.end_side(), range.end()).ok()?)?;
            let start = model.source_lines().iter().find(|line| line.position() == start)?;
            let end = model.source_lines().iter().find(|line| line.position() == end)?;
            (start.hunk_index() == end.hunk_index()).then_some(start.hunk_index())
        });
        let (label, action) = match target.group {
            ChangeGroup::Unstaged => ("暂存", DiffAction::stage()),
            ChangeGroup::Staged => ("取消暂存", DiffAction::unstage()),
        };
        let discardable = target.group == ChangeGroup::Unstaged
            && self.model.as_ref().is_some_and(|model| model.is_discardable());
        let mut snapshot = self.tool_snapshot();
        snapshot.hunk = hunk;
        snapshot.available[1] = hunk.is_some()
            && self.model.as_ref().is_some_and(|model| model.partial_error().is_none());
        let busy = self.writes_busy(cx);
        let scopes = [
            (ToolScope::lines(), format!("选中行（{} 个变更行）", self.selected_count)),
            (
                ToolScope::hunk(),
                hunk.map_or_else(
                    || "选区所在变更块（需选在同一块内）".to_owned(),
                    |index| format!("选区所在变更块（第 {} 块）", index + 1),
                ),
            ),
        ];
        for (scope, name) in &scopes {
            let view = cx.weak_entity();
            let snapshot = snapshot.clone();
            let scope = *scope;
            menu = menu.item(
                PopupMenuItem::new(format!("{label}{name}"))
                    .disabled(busy || !snapshot.scope_available(scope))
                    .on_click(move |_, window, cx| {
                        let _ = view.update(cx, |this, cx| {
                            if this.restore_tool_snapshot(&snapshot, cx) {
                                this.request_change(action, scope, window, cx);
                            }
                        });
                    }),
            );
        }
        if discardable {
            menu = menu.item(PopupMenuItem::separator());
            for (scope, name) in scopes {
                let view = cx.weak_entity();
                let snapshot = snapshot.clone();
                menu = menu.item(
                    PopupMenuItem::new(format!("丢弃{name}…"))
                        .icon(IconName::Trash)
                        .disabled(busy || !snapshot.scope_available(scope))
                        .on_click(move |_, window, cx| {
                            let _ = view.update(cx, |this, cx| {
                                if this.restore_tool_snapshot(&snapshot, cx) {
                                    this.request_change(DiffAction::discard(), scope, window, cx);
                                }
                            });
                        }),
                );
            }
        }
        menu
    }

    pub fn request_change(
        &mut self,
        action: DiffAction,
        scope: ToolScope,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.writes_busy(cx) || !self.scope_available(scope) {
            return;
        }
        let Some(target) = &self.target else {
            return;
        };
        if action == DiffAction::discard()
            && self.model.as_ref().is_none_or(|model| !model.is_discardable())
        {
            return;
        }
        let Some(patch_scope) = self.scope_patch(scope) else {
            return;
        };
        let request = DiffRequest::new(
            self.directory.clone(),
            target.path.clone(),
            target.group,
            self.patch.clone(),
            patch_scope,
            action,
        );
        let owner = self.owner.clone();
        if action == DiffAction::discard() {
            if target.group != ChangeGroup::Unstaged {
                return;
            }
            let title = format!("丢弃{}？", scope.label());
            let description = format!(
                "将丢弃 {} 的{}，仅影响未暂存变更。执行前会创建持久备份，可从「恢复丢弃」菜单恢复（重启后仍保留）。未跟踪文件可能被移除；恢复前会检查文件是否已变化。操作期间请暂停编辑该文件。",
                target.path.display(),
                scope.label()
            );
            window.open_alert_dialog(cx, move |dialog, _, _| {
                let owner = owner.clone();
                let request = request.clone();
                dialog
                    .title(title.clone())
                    .description(description.clone())
                    .confirm()
                    .ok_text("丢弃变更")
                    .ok_variant(ButtonVariant::Danger)
                    .cancel_text("取消")
                    .on_ok(move |_, window, cx| {
                        dispatch_change(owner.clone(), request.clone(), window, cx);
                        true
                    })
            });
        } else {
            dispatch_change(owner, request, window, cx);
        }
    }

    pub fn choose_hunk(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(model) = &self.model else {
            return;
        };
        let Some(line) = model
            .source_lines()
            .iter()
            .find(|line| line.hunk_index() == index && line.is_changed())
        else {
            return;
        };
        let Some(file) = self.state.read(cx).files().first() else {
            return;
        };
        let (side, number) = match (line.modified_line(), line.original_line()) {
            (Some(number), _) => (DiffSide::Modified, number),
            (_, Some(number)) => (DiffSide::Original, number),
            _ => return,
        };
        let position = DiffLinePosition::new(file.path().clone(), side, number as usize);
        self.current_hunk = Some(index);
        self.location = Some(position.clone());
        self.state.update(cx, |state, cx| state.scroll_to_line(position, cx));
        cx.notify();
    }

    pub fn move_hunk(&mut self, backwards: bool, cx: &mut Context<Self>) {
        let count = self.model.as_ref().map_or(0, |model| model.hunks().len());
        if count == 0 {
            return;
        }
        let current = self.current_hunk.unwrap_or(0);
        let next = if backwards { (current + count - 1) % count } else { (current + 1) % count };
        self.choose_hunk(next, cx);
    }
}
