use agent_client_protocol::v1::schema::{AvailableCommand, AvailableCommandInput};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::ui::ChatView;
use crate::ui::palette::Palette;

#[derive(Debug, Default)]
pub struct SlashMenu {
    selected: usize,
    dismissed: bool,
    focused: bool,
    scroll: ScrollHandle,
}

#[derive(Clone, Copy, Debug)]
pub enum CommandMenuAction {
    Previous,
    Next,
    Complete,
    Dismiss,
}

impl SlashMenu {
    pub fn reset(&mut self) {
        self.selected = 0;
        self.dismissed = false;
        self.scroll.set_offset(point(px(0.), px(0.)));
    }

    pub fn focus(&mut self) {
        self.focused = true;
        self.reset();
    }

    pub fn blur(&mut self) {
        self.focused = false;
    }
}

impl ChatView {
    fn command_candidates(&self, cx: &App) -> Option<Vec<&AvailableCommand>> {
        if !self.command_menu.focused
            || self.command_menu.dismissed
            || self.disconnected
            || self.loading_session.is_some()
            || self.creating_session.is_some()
            || self.choosing_directory
        {
            return None;
        }
        let input = self.input.read(cx);
        let text = input.value();
        let query = text.strip_prefix('/')?;
        if query.chars().any(|character| character.is_whitespace() || character == '/')
            || !input.selected_range().is_empty()
            || input.cursor() != text.len()
        {
            return None;
        }
        let query = query.to_lowercase();
        Some(
            self.chat
                .available_commands()
                .iter()
                .filter(|command| command.name.to_lowercase().starts_with(&query))
                .collect(),
        )
    }

    pub fn complete_selected_command(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(candidates) = self.command_candidates(cx) else {
            return false;
        };
        let Some(command) = candidates.get(self.command_menu.selected) else {
            return false;
        };
        let name = command.name.clone();
        self.select_command(&name, window, cx);
        true
    }

    fn select_command(&mut self, name: &str, window: &mut Window, cx: &mut Context<Self>) {
        let text = format!("/{name} ");
        let end = text.len();
        self.input.update(cx, |input, cx| {
            input.replace_all(text, window, cx);
            input.set_selected_range(end..end, cx);
            input.focus(window, cx);
        });
        self.command_menu.dismissed = true;
        cx.notify();
    }

    pub fn command_menu_action(
        &mut self,
        action: CommandMenuAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(count) = self.command_candidates(cx).map(|candidates| candidates.len()) else {
            return;
        };
        if self.input.update(cx, |input, cx| input.marked_text_range(window, cx).is_some()) {
            return;
        }
        match action {
            CommandMenuAction::Previous if count > 0 => {
                self.command_menu.selected = (self.command_menu.selected + count - 1) % count;
                self.command_menu.scroll.scroll_to_item(self.command_menu.selected);
            }
            CommandMenuAction::Next if count > 0 => {
                self.command_menu.selected = (self.command_menu.selected + 1) % count;
                self.command_menu.scroll.scroll_to_item(self.command_menu.selected);
            }
            CommandMenuAction::Complete if count > 0 => {
                self.complete_selected_command(window, cx);
            }
            CommandMenuAction::Dismiss => self.command_menu.dismissed = true,
            _ => return,
        }
        window.prevent_default();
        cx.stop_propagation();
        cx.notify();
    }

    pub fn render_commands(&self, p: Palette, cx: &mut Context<Self>) -> Option<AnyElement> {
        let candidates = self.command_candidates(cx)?;
        Some(
            div()
                .id("slash-commands")
                .absolute()
                .bottom_full()
                .left_0()
                .right_0()
                .mb_2()
                .rounded(rems(0.75))
                .border_1()
                .border_color(p.border())
                .bg(p.input())
                .shadow_lg()
                .occlude()
                .child(div().px_3().py_2().text_xs().text_color(p.muted()).child("可用命令"))
                .child(
                    div()
                        .id("slash-command-list")
                        .max_h(rems(15.))
                        .overflow_y_scroll()
                        .track_scroll(&self.command_menu.scroll)
                        .px_1()
                        .pb_1()
                        .when(candidates.is_empty(), |list| {
                            list.child(div().p_3().text_color(p.muted()).child(
                                if self.session_id.is_none() {
                                    "新建会话后可获取可用命令"
                                } else if self.chat.available_commands().is_empty() {
                                    "当前会话暂无可用命令"
                                } else {
                                    "没有匹配的命令"
                                },
                            ))
                        })
                        .children(candidates.into_iter().enumerate().map(|(index, command)| {
                            let name = command.name.clone();
                            let hint = match &command.input {
                                Some(AvailableCommandInput::Unstructured(input)) => {
                                    Some(input.hint.clone())
                                }
                                _ => None,
                            };
                            let details = format!(
                                "/{}{}\n{}",
                                command.name,
                                hint.map(|hint| format!(" {hint}")).unwrap_or_default(),
                                command.description,
                            );
                            div()
                                .id(("slash-command", index))
                                .flex()
                                .items_center()
                                .gap_3()
                                .w_full()
                                .min_w_0()
                                .px_3()
                                .py_2()
                                .rounded(rems(0.5))
                                .when(index == self.command_menu.selected, |row| row.bg(p.hover()))
                                .hover(|row| row.bg(p.hover()))
                                .aria_label(format!("/{}：{}", command.name, command.description))
                                .tooltip(move |window, cx| {
                                    let details = details.clone();
                                    Tooltip::element(move |_, _| {
                                        div()
                                            .max_w(rems(30.))
                                            .whitespace_normal()
                                            .child(details.clone())
                                    })
                                    .build(window, cx)
                                })
                                .child(
                                    div()
                                        .flex_shrink_0()
                                        .min_w_0()
                                        .max_w(relative(0.4))
                                        .truncate()
                                        .line_clamp(1)
                                        .font_weight(FontWeight::MEDIUM)
                                        .child(format!("/{}", command.name)),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .truncate()
                                        .line_clamp(1)
                                        .text_xs()
                                        .text_color(p.muted())
                                        .child(command.description.clone()),
                                )
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |this, _, window, cx| {
                                        this.select_command(&name, window, cx);
                                        window.prevent_default();
                                        cx.stop_propagation();
                                    }),
                                )
                        })),
                )
                .child(
                    div()
                        .px_3()
                        .py_2()
                        .border_t_1()
                        .border_color(p.border())
                        .text_size(rems(0.6875))
                        .text_color(p.muted())
                        .child("↑ ↓ 选择，Enter / Tab 填入，Esc 关闭"),
                )
                .into_any_element(),
        )
    }
}
