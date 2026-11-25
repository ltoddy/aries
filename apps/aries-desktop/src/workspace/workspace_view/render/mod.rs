use gpui_kit::component::{ActiveTheme, h_resizable, resizable_panel};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::workspace::files::SaveFile;
use crate::workspace::settings::palette::Palette;
use crate::workspace::workspace_view::{
    LayoutSizes, Shell, SidebarLayout, SidebarPanel, TabCommand, WorkspaceView,
};
use crate::{
    CloseTab, NewSession, NextTab, NextUserMessage, OpenSettings, PreviousTab, PreviousUserMessage,
    ShowFilesSidebar, ShowGitSidebar, ShowSessionsSidebar, ToggleSidebar, ToggleTerminal,
};

mod content;
mod sidebar;
mod terminal;
mod title_bar;

impl Render for WorkspaceView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = Palette::new(cx.theme().mode);
        let sizes = LayoutSizes::new(cx.theme().font_size, window.viewport_size());
        let sidebar = SidebarLayout::new(
            self.sidebar.visible,
            self.layout.read(cx).sizes().first().copied(),
            &sizes,
        );
        if !sidebar.is_visible() && self.sidebar.focus_handle.contains_focused(window, cx) {
            cx.defer_in(window, |this, window, cx| {
                if this.sidebar.focus_handle.contains_focused(window, cx) {
                    this.sidebar.toggle_focus_handle.focus(window, cx);
                }
            });
        }
        let style = div().size_full().style().clone();
        let shell = Shell::new(&sizes, sidebar, palette, style);
        let body = self.render_body(&shell, window, cx);
        let title_bar = self.render_title_bar(&shell, cx);
        self.render_shell(title_bar, body, &shell, cx)
    }
}

impl WorkspaceView {
    fn render_body(
        &self,
        shell: &Shell,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let content = self.render_content(shell, cx);
        let content = self.render_terminal_layout(content, window, cx);
        let main = div()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .flex_col()
            .pr_1p5()
            .pb_1p5()
            .when(!shell.sidebar.is_visible(), |main| main.pl_1p5())
            .child(
                div()
                    .relative()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .rounded(cx.theme().radius_tokens().lg)
                    .bg(shell.palette.background())
                    .overflow_hidden()
                    .child(content),
            );
        div()
            .flex()
            .items_stretch()
            .flex_1()
            .min_h_0()
            .child(if shell.sidebar.is_visible() {
                h_resizable("layout")
                    .with_state(&self.layout)
                    .child(
                        resizable_panel()
                            .size(shell.sizes.sidebar_default())
                            .size_range(shell.sidebar.minimum()..shell.sidebar.maximum())
                            .child(
                                div()
                                    .size_full()
                                    .track_focus(&self.sidebar.focus_handle)
                                    .child(self.render_sidebar(shell, window, cx)),
                            ),
                    )
                    .child(resizable_panel().child(main))
                    .into_any_element()
            } else {
                main.into_any_element()
            })
            .into_any_element()
    }

    fn render_shell(
        &self,
        title_bar: AnyElement,
        body: AnyElement,
        shell: &Shell,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .size_full()
            .track_focus(&self.focus_handle)
            .capture_action(cx.listener(|this, _: &SaveFile, window, cx| {
                if let Some(id) = this.active_id() {
                    this.save_file_tab(id, window, cx);
                }
                cx.stop_propagation();
            }))
            .on_action(cx.listener(|this, _: &OpenSettings, window, cx| {
                this.open_settings(window, cx);
            }))
            .on_action(cx.listener(|this, _: &ToggleTerminal, window, cx| {
                this.toggle_terminal(window, cx);
            }))
            .on_action(cx.listener(|this, _: &ToggleSidebar, window, cx| {
                this.toggle_sidebar(window, cx);
            }))
            .on_action(cx.listener(|this, _: &ShowSessionsSidebar, window, cx| {
                this.select_sidebar_panel(SidebarPanel::sessions(), window, cx);
            }))
            .on_action(cx.listener(|this, _: &ShowFilesSidebar, window, cx| {
                this.select_sidebar_panel(SidebarPanel::files(), window, cx);
            }))
            .on_action(cx.listener(|this, _: &ShowGitSidebar, window, cx| {
                this.select_sidebar_panel(SidebarPanel::git(), window, cx);
            }))
            .on_action(cx.listener(|this, _: &CloseTab, window, cx| {
                if let Some(id) = this.active_id() {
                    this.execute_tab_command(TabCommand::close(id), window, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &PreviousTab, window, cx| {
                this.activate_previous(window, cx);
            }))
            .on_action(cx.listener(|this, _: &NextTab, window, cx| {
                this.activate_next(window, cx);
            }))
            .on_action(cx.listener(|this, _: &PreviousUserMessage, _, cx| {
                if this.is_sessions_visible()
                    && let Some(view) = this.active_view()
                {
                    view.update(cx, |view, cx| view.jump_user_message(false, cx));
                }
            }))
            .on_action(cx.listener(|this, _: &NextUserMessage, _, cx| {
                if this.is_sessions_visible()
                    && let Some(view) = this.active_view()
                {
                    view.update(cx, |view, cx| view.jump_user_message(true, cx));
                }
            }))
            .on_action(cx.listener(|this, _: &NewSession, window, cx| {
                this.new_session(window, cx);
            }))
            .flex()
            .flex_col()
            .bg(shell.palette.sidebar().opacity(if cfg!(target_os = "macos") { 0.82 } else { 1. }))
            .text_color(shell.palette.foreground())
            .font_family(cx.theme().font_family.clone())
            .text_sm()
            .child(title_bar)
            .child(body)
            .into_any_element()
    }
}
