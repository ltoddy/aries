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
use gpui_kit::component::menu::{DropdownMenu, PopupMenuItem};
use gpui_kit::component::{ActiveTheme, Disableable, Sizable, WindowExt};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::workspace::git::diff_operations::{DiffAction, DiffRequest, DiscardBackup};
use crate::workspace::git::diff_patch::{
    LineEndpoint, LineSelection, PatchDirection, PatchScope, SourceSide,
};
use crate::workspace::git::diff_view::GitDiffView;
use crate::workspace::git::repository::ChangeGroup;

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
    scope: ToolScope,
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
            scope: view.scope,
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
        self.scope = snapshot.scope;
        self.current_hunk = snapshot.hunk;
        self.location = snapshot.location.clone();
        cx.notify();
        true
    }

    pub fn reset_tools(&mut self) {
        self.model = None;
        self.tool_error = None;
        self.scope = ToolScope::file();
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

    pub fn initialize_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.search_input.is_some() {
            return;
        }
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("搜索 diff（区分大小写）"));
        self.search_subscription = Some(cx.subscribe(&input, |this, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                this.update_search(cx);
            }
        }));
        self.search_input = Some(input);
    }

    pub fn show_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search_open = true;
        if let Some(input) = &self.search_input {
            input.focus_handle(cx).focus(window, cx);
        }
        self.update_search(cx);
        cx.notify();
    }

    pub fn close_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search_open = false;
        self.search_revision = self.search_revision.wrapping_add(1);
        self.search_task = None;
        self.search_loading = false;
        self.matches.clear();
        self.match_index = None;
        self.decorations.clear();
        self.state.focus_handle(cx).focus(window, cx);
        cx.notify();
    }

    pub fn update_search(&mut self, cx: &mut Context<Self>) {
        self.search_revision = self.search_revision.wrapping_add(1);
        self.search_task = None;
        self.matches.clear();
        self.match_index = None;
        self.decorations.clear();
        self.search_loading = false;
        if !self.search_open {
            return;
        }
        let Some(model) = self.model.clone() else {
            return;
        };
        let query =
            self.search_input.as_ref().map(|input| input.read(cx).value()).unwrap_or_default();
        if query.is_empty() {
            cx.notify();
            return;
        }
        let Some(file) = self.state.read(cx).files().first() else {
            return;
        };
        let path = file.path().clone();
        let revision = self.search_revision;
        self.search_loading = true;
        self.search_task = Some(cx.spawn(async move |view, cx| {
            let matches = cx
                .background_spawn(async move {
                    let mut matches = Vec::new();
                    for line in model.source_lines() {
                        if !line.content().contains(query.as_str()) {
                            continue;
                        }
                        let (side, number) = match (line.original_line(), line.modified_line()) {
                            (_, Some(number)) => (DiffSide::Modified, number),
                            (Some(number), _) => (DiffSide::Original, number),
                            _ => continue,
                        };
                        matches.push(DiffLinePosition::new(path.clone(), side, number as usize));
                    }
                    matches
                })
                .await;
            let _ = view.update(cx, |this, cx| {
                if this.search_revision != revision {
                    return;
                }
                this.search_loading = false;
                this.matches = matches;
                this.match_index = (!this.matches.is_empty()).then_some(0);
                this.update_decorations(cx);
                this.reveal_match(cx);
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn update_decorations(&mut self, cx: &App) {
        self.decorations = self
            .matches
            .iter()
            .take(if self.matches.len() <= 128 { self.matches.len() } else { 0 })
            .map(|position| {
                DiffDecoration::new(DiffLineRange::new(
                    position.path().clone(),
                    position.side(),
                    position.line(),
                    position.line(),
                ))
                .with_background(cx.theme().primary.opacity(0.18))
            })
            .collect();
        if let Some(position) = self.match_index.and_then(|index| self.matches.get(index)) {
            self.decorations.push(
                DiffDecoration::new(DiffLineRange::new(
                    position.path().clone(),
                    position.side(),
                    position.line(),
                    position.line(),
                ))
                .with_background(cx.theme().primary.opacity(0.4)),
            );
        }
    }

    fn reveal_match(&mut self, cx: &mut Context<Self>) {
        if let Some(position) = self.match_index.and_then(|index| self.matches.get(index)).cloned()
        {
            self.set_location(position.clone(), cx);
            self.state.update(cx, |state, cx| state.scroll_to_line(position, cx));
        }
    }

    pub fn move_match(&mut self, backwards: bool, cx: &mut Context<Self>) {
        let count = self.matches.len();
        if count == 0 {
            return;
        }
        let current = self.match_index.unwrap_or(0);
        self.match_index =
            Some(if backwards { (current + count - 1) % count } else { (current + 1) % count });
        self.update_decorations(cx);
        self.reveal_match(cx);
        cx.notify();
    }

    pub fn render_search(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(input) = &self.search_input else {
            return div().into_any_element();
        };
        div()
            .flex()
            .items_center()
            .gap_2()
            .px_3()
            .py_2()
            .flex_shrink_0()
            .child(div().flex_1().min_w_0().child(Input::new(input).small()))
            .child(
                div()
                    .flex_shrink_0()
                    .text_xs()
                    .font_family(cx.theme().mono_font_family.clone())
                    .text_color(cx.theme().muted_foreground)
                    .child(if self.search_loading {
                        "正在搜索".to_owned()
                    } else {
                        format!(
                            "{} / {}",
                            self.match_index.map_or(0, |index| index + 1),
                            self.matches.len()
                        )
                    }),
            )
            .child(
                Button::new("previous-diff-match")
                    .small()
                    .ghost()
                    .compact()
                    .icon(IconName::ChevronUp)
                    .accessibility_label("上一个匹配")
                    .disabled(self.matches.is_empty())
                    .on_click(cx.listener(|this, _, _, cx| this.move_match(true, cx))),
            )
            .child(
                Button::new("next-diff-match")
                    .small()
                    .ghost()
                    .compact()
                    .icon(IconName::ChevronDown)
                    .accessibility_label("下一个匹配")
                    .disabled(self.matches.is_empty())
                    .on_click(cx.listener(|this, _, _, cx| this.move_match(false, cx))),
            )
            .child(
                Button::new("close-diff-search")
                    .small()
                    .ghost()
                    .compact()
                    .icon(IconName::X)
                    .accessibility_label("关闭搜索")
                    .on_click(cx.listener(|this, _, window, cx| this.close_search(window, cx))),
            )
            .into_any_element()
    }

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

    fn scope_patch(&self, scope: ToolScope) -> Option<PatchScope> {
        match scope {
            ToolScope::File => Some(PatchScope::file()),
            ToolScope::Hunk => self.current_hunk.map(PatchScope::hunk),
            ToolScope::Lines => self.selected_scope.clone(),
        }
    }

    pub fn controls_busy(&self, cx: &App) -> bool {
        self.loading
            || self.owner.upgrade().is_none_or(|owner| {
                let panel = owner.read(cx).git_panel();
                panel.directory() != self.directory || panel.is_index_busy()
            })
    }

    fn writes_busy(&self, cx: &App) -> bool {
        self.error.is_some() || self.controls_busy(cx)
    }

    fn scope_available(&self, scope: ToolScope) -> bool {
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
        let view = cx.weak_entity();
        let selected = self.scope;
        let snapshot = self.tool_snapshot();
        let menu_snapshot = snapshot.clone();
        div()
            .flex()
            .items_center()
            .gap_1()
            .flex_shrink_0()
            .child(
                Button::new("diff-operation-scope")
                    .small()
                    .ghost()
                    .compact()
                    .label(selected.label())
                    .icon(IconName::ChevronDown)
                    .tooltip("选中行需点击行号，Shift 扩展；仅操作真正变更的行")
                    .dropdown_menu(move |menu, _, cx| {
                        let restored = view
                            .upgrade()
                            .is_some_and(|view| menu_snapshot.is_current(view.read(cx)));
                        let mut menu = menu;
                        for scope in [ToolScope::file(), ToolScope::hunk(), ToolScope::lines()] {
                            let enabled = restored && menu_snapshot.scope_available(scope);
                            let view = view.clone();
                            let snapshot = menu_snapshot.clone();
                            menu = menu.item(
                                PopupMenuItem::new(scope.label())
                                    .checked(selected == scope)
                                    .disabled(!enabled)
                                    .on_click(move |_, _, cx| {
                                        let _ = view.update(cx, |this, cx| {
                                            if this.restore_tool_snapshot(&snapshot, cx) {
                                                this.scope = scope;
                                                cx.notify();
                                            }
                                        });
                                    }),
                            );
                        }
                        menu
                    }),
            )
            .child(
                Button::new("diff-index-operation")
                    .small()
                    .compact()
                    .secondary()
                    .label(label)
                    .disabled(self.writes_busy(cx) || !self.scope_available(self.scope))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if this.restore_tool_snapshot(&snapshot, cx) {
                            this.request_change(action, window, cx);
                        }
                    })),
            )
            .into_any_element()
    }

    fn request_change(&mut self, action: DiffAction, window: &mut Window, cx: &mut Context<Self>) {
        if self.writes_busy(cx) || !self.scope_available(self.scope) {
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
        let Some(scope) = self.scope_patch(self.scope) else {
            return;
        };
        let request = DiffRequest::new(
            self.directory.clone(),
            target.path.clone(),
            target.group,
            self.patch.clone(),
            scope,
            action,
        );
        let owner = self.owner.clone();
        if action == DiffAction::discard() {
            if target.group != ChangeGroup::Unstaged {
                return;
            }
            let title = format!("丢弃{}？", self.scope.label());
            let description = format!(
                "将丢弃 {} 的{}，仅影响未暂存变更。执行前会创建持久备份，可从「恢复丢弃」菜单恢复（重启后仍保留）。未跟踪文件可能被移除；恢复前会检查文件是否已变化。操作期间请暂停编辑该文件。",
                target.path.display(),
                self.scope.label()
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

    fn copy_patch(&mut self, scope: ToolScope, window: &mut Window, cx: &mut Context<Self>) {
        let result = if scope == ToolScope::File {
            Ok(self.patch.as_ref().clone())
        } else {
            self.model
                .as_ref()
                .zip(self.scope_patch(scope))
                .ok_or_else(ToolError::selection_required)
                .and_then(|(model, scope)| {
                    model.selected_patch(&scope, PatchDirection::forward()).map_err(ToolError::from)
                })
        };
        match result {
            Ok(patch) => {
                cx.write_to_clipboard(ClipboardItem::new_string(patch));
                self.tool_error = None;
            },
            Err(error) => {
                self.tool_error = Some(format!("无法复制 patch：{}", details(&error)));
                report("无法复制 patch", &error, window, cx);
            },
        }
        cx.notify();
    }

    fn choose_hunk(&mut self, index: usize, cx: &mut Context<Self>) {
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

    fn move_hunk(&mut self, backwards: bool, cx: &mut Context<Self>) {
        let count = self.model.as_ref().map_or(0, |model| model.hunks().len());
        if count == 0 {
            return;
        }
        let current = self.current_hunk.unwrap_or(0);
        let next = if backwards { (current + count - 1) % count } else { (current + 1) % count };
        self.choose_hunk(next, cx);
    }

    pub fn render_tools(&self, cx: &mut Context<Self>) -> AnyElement {
        let view = cx.weak_entity();
        let current = self.current_hunk;
        let count = self.model.as_ref().map_or(0, |model| model.hunks().len());
        let hunk_view = view.clone();
        let more_view = view.clone();
        let restore_view = view;
        let hunk_snapshot = self.tool_snapshot();
        let more_snapshot = hunk_snapshot.clone();
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
                Button::new("diff-more-tools")
                    .small()
                    .ghost()
                    .compact()
                    .label("更多")
                    .icon(IconName::ChevronDown)
                    .dropdown_menu(move |menu, _, cx| {
                        let Some(view) = more_view.upgrade() else {
                            return menu;
                        };
                        let restored = more_snapshot.is_current(view.read(cx));
                        let this = view.read(cx);
                        let discard_enabled = restored
                            && !this.writes_busy(cx)
                            && this
                                .target
                                .as_ref()
                                .is_some_and(|target| target.group == ChangeGroup::Unstaged)
                            && more_snapshot.scope_available(more_snapshot.scope)
                            && this.model.as_ref().is_some_and(|model| model.is_discardable());
                        let discard_label = format!("丢弃{}…", more_snapshot.scope.label());
                        let mut menu = menu;
                        for (scope, label) in [
                            (ToolScope::file(), "复制文件 patch"),
                            (ToolScope::hunk(), "复制当前块 patch"),
                            (ToolScope::lines(), "复制选中行 patch"),
                        ] {
                            let enabled = restored && more_snapshot.scope_available(scope);
                            let view = more_view.clone();
                            let snapshot = more_snapshot.clone();
                            menu =
                                menu.item(PopupMenuItem::new(label).disabled(!enabled).on_click(
                                    move |_, window, cx| {
                                        let _ = view.update(cx, |this, cx| {
                                            if this.restore_tool_snapshot(&snapshot, cx) {
                                                this.copy_patch(scope, window, cx);
                                            }
                                        });
                                    },
                                ));
                        }
                        menu = menu.item(PopupMenuItem::separator());
                        let view = more_view.clone();
                        let location_snapshot = more_snapshot.clone();
                        menu = menu.item(
                            PopupMenuItem::new("复制 diff 行位置")
                                .disabled(!restored || more_snapshot.location.is_none())
                                .on_click(move |_, _, cx| {
                                    let _ = view.update(cx, |this, cx| {
                                        if !this.restore_tool_snapshot(&location_snapshot, cx) {
                                            return;
                                        }
                                        if let Some(position) = &location_snapshot.location {
                                            let side = if position.side() == DiffSide::Original {
                                                "原始侧"
                                            } else {
                                                "修改侧"
                                            };
                                            cx.write_to_clipboard(ClipboardItem::new_string(
                                                format!(
                                                    "{}:{} ({side})",
                                                    position.path(),
                                                    position.line()
                                                ),
                                            ));
                                        }
                                    });
                                }),
                        );
                        let view = more_view.clone();
                        let snapshot = more_snapshot.clone();
                        menu.item(PopupMenuItem::separator()).item(
                            PopupMenuItem::new(discard_label)
                                .icon(IconName::Trash)
                                .disabled(!discard_enabled)
                                .on_click(move |_, window, cx| {
                                    let _ = view.update(cx, |this, cx| {
                                        if this.restore_tool_snapshot(&snapshot, cx) {
                                            this.request_change(DiffAction::discard(), window, cx);
                                        }
                                    });
                                }),
                        )
                    }),
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
                    "已选 {} 个变更行{}",
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
    #[error("请先选择包含变更的行或块")]
    SelectionRequired,
    #[error("line number {line} exceeds supported patch coordinates")]
    LineNumber { line: usize, source: std::num::TryFromIntError },
}

impl ToolError {
    fn selection_required() -> Self {
        Self::SelectionRequired
    }
    fn line_number(line: usize, source: std::num::TryFromIntError) -> Self {
        Self::LineNumber { line, source }
    }
}

impl From<PatchError> for ToolError {
    fn from(source: PatchError) -> Self {
        Self::Patch { source }
    }
}
