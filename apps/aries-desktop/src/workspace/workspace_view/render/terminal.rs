use gpui_kit::base::GlobalState;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::{ActiveTheme, Sizable, resizable_panel, v_resizable};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::ToggleTerminal;
use crate::terminal::TerminalView;
use crate::workspace::workspace_view::{LayoutSizes, WorkspaceView};

impl WorkspaceView {
    pub fn render_terminal_layout(
        &self,
        content: AnyElement,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(terminal) = self.terminal.view.as_ref().filter(|_| self.terminal.is_visible())
        else {
            return content;
        };
        let panel = self.render_terminal(terminal, cx).into_any_element();
        if self.terminal.is_zoomed() {
            return panel;
        }
        let terminal_sizes =
            LayoutSizes::new(cx.theme().font_size, window.viewport_size()).terminal();
        v_resizable("main-terminal-layout")
            .with_state(&self.terminal.layout)
            .child(
                resizable_panel()
                    .size(terminal_sizes.content_height())
                    .size_range(terminal_sizes.content_minimum()..Pixels::MAX)
                    .child(content),
            )
            .child(
                resizable_panel()
                    .size(terminal_sizes.split())
                    .size_range(terminal_sizes.minimum()..terminal_sizes.maximum())
                    .child(panel),
            )
            .into_any_element()
    }

    pub fn render_terminal(
        &self,
        terminal: &Entity<TerminalView>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let directory = terminal.read(cx).directory().display().to_string();
        let zoom_label = if self.terminal.is_zoomed() { "还原" } else { "满屏" };
        div()
            .id("terminal-panel")
            .on_mouse_down(MouseButton::Left, |_, _, cx| {
                GlobalState::suppress_text_selection(cx);
            })
            .size_full()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .overflow_hidden()
            .child(
                div()
                    .flex()
                    .flex_shrink_0()
                    .items_center()
                    .gap_2()
                    .px_3()
                    .py_1()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child("终端")
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_color(cx.theme().muted_foreground)
                            .child(directory),
                    )
                    .child(
                        Button::new("zoom-terminal")
                            .small()
                            .ghost()
                            .label(zoom_label)
                            .accessibility_label(if self.terminal.is_zoomed() {
                                "还原终端面板"
                            } else {
                                "终端铺满工作区"
                            })
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.toggle_terminal_zoom(window, cx)
                            })),
                    )
                    .child(
                        Button::new("hide-terminal")
                            .small()
                            .ghost()
                            .label("隐藏")
                            .tooltip_with_action("隐藏终端", &ToggleTerminal, None)
                            .on_click(
                                cx.listener(|this, _, window, cx| this.toggle_terminal(window, cx)),
                            ),
                    ),
            )
            .child(div().flex_1().min_h_0().min_w_0().p_2().child(terminal.clone()))
    }
}
