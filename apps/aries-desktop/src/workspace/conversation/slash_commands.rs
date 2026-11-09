use agent_client_protocol::v1::schema::{AvailableCommand, AvailableCommandInput};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::workspace::settings::palette::Palette;

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

    pub fn selected(&self) -> usize {
        self.selected
    }
    pub fn is_open(&self) -> bool {
        self.focused && !self.dismissed
    }
    pub fn dismiss(&mut self) {
        self.dismissed = true;
    }
    pub fn move_selection(&mut self, action: CommandMenuAction, count: usize) {
        self.selected = match action {
            CommandMenuAction::Previous => (self.selected + count - 1) % count,
            CommandMenuAction::Next => (self.selected + 1) % count,
            _ => return,
        };
        self.scroll.scroll_to_item(self.selected);
    }
    pub fn blur(&mut self) {
        self.focused = false;
    }
}

type CommandSelection = Box<dyn Fn(&str, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct CommandMenu {
    candidates: Vec<AvailableCommand>,
    selected: usize,
    scroll: ScrollHandle,
    empty: &'static str,
    palette: Palette,
    on_select: CommandSelection,
}
impl CommandMenu {
    pub fn from_commands(
        menu: &SlashMenu,
        candidates: Vec<&AvailableCommand>,
        empty: &'static str,
        p: Palette,
        on_select: impl Fn(&str, &mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            candidates: candidates.into_iter().cloned().collect(),
            selected: menu.selected,
            scroll: menu.scroll.clone(),
            empty,
            palette: p,
            on_select: Box::new(on_select),
        }
    }
}
impl RenderOnce for CommandMenu {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let Self { candidates, selected, scroll, empty, palette: p, on_select } = self;
        let on_select = std::rc::Rc::new(on_select);
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
                    .track_scroll(&scroll)
                    .px_1()
                    .pb_1()
                    .when(candidates.is_empty(), |list| {
                        list.child(div().p_3().text_color(p.muted()).child(empty))
                    })
                    .children(candidates.into_iter().enumerate().map(|(index, command)| {
                        let name = command.name.clone();
                        let on_select = on_select.clone();
                        let hint = match &command.input {
                            Some(AvailableCommandInput::Unstructured(input)) => {
                                Some(input.hint.as_str())
                            },
                            _ => None,
                        };
                        let details = SharedString::from(format!(
                            "/{}{}\n{}",
                            command.name,
                            hint.map(|hint| format!(" {hint}")).unwrap_or_default(),
                            command.description,
                        ));
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
                            .when(index == selected, |row| row.bg(p.hover()))
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
                            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                                on_select(&name, window, cx);
                                window.prevent_default();
                                cx.stop_propagation();
                            })
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
            .into_any_element()
    }
}
